"""Harbor Files: play big videos and music from the phone without copying first.

The phone shares over FTP. gvfs can't seek in FTP files, so a player reading
through it could only start at the beginning, and Harbor's usual way (copy
to ~/.cache, then open) makes you wait for the whole file.

Streaming: Harbor runs a tiny HTTP relay on 127.0.0.1 only. The player gets
http://127.0.0.1:<port>/<random token>/<name>; each request (with a Range for
seeking) becomes one FTP RETR with REST from that byte on the phone, so the
video starts at once and skipping ahead fetches only that part.

Safety:
* Bound to 127.0.0.1, never the network. Each file gets its own random
  256-bit token; anything else is 404. Only GET and HEAD.
* The phone PIN stays in this process's memory (never saved or logged) and
  never reaches the player: the player only sees the localhost link.
* Requests are not logged (the URL holds the token).
"""

import ftplib
import os
import secrets
import threading
from collections import OrderedDict
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import quote, unquote, urlsplit

VIDEO_MIN = 5 * 1024 * 1024        # ask for videos at least this big
AUDIO_MIN = 50 * 1024 * 1024       # ...and music at least this big
STREAM_MIN = VIDEO_MIN
CHOICES = ("ask", "stream", "download")
MAX_ITEMS = 32                     # links kept; older ones stop working
CHUNK = 256 * 1024
FTP_TIMEOUT = 20

_pins = {}                         # (host, port) -> PIN, memory only
_lock = threading.Lock()


# ------------------------------------------------------------------ PINs

def remember_pin(host, port, pin):
    if host and pin:
        with _lock:
            _pins[(host, int(port or 21))] = pin


def pin_for(host, port):
    with _lock:
        return _pins.get((host, int(port or 21)))


def forget_pin(host, port):
    with _lock:
        _pins.pop((host, int(port or 21)), None)


# ------------------------------------------------------------------ helpers

def is_media(content_type):
    ct = content_type or ""
    return ct.startswith("video/") or ct.startswith("audio/")


def wants_choice(content_type, size):
    """Stream or download? Asked for videos over 5 MB and music over 50 MB."""
    ct = content_type or ""
    if ct.startswith("video/"):
        return (size or 0) >= VIDEO_MIN
    return ct.startswith("audio/") and (size or 0) >= AUDIO_MIN


# ------------------------------------------------------------------ temporary copies

def play_dir():
    """Where "Download and play" copies live until the player closes them."""
    from gi.repository import GLib
    return os.path.join(GLib.get_user_cache_dir(), "harbor", "play")


def open_by_anyone(path):
    """True when any process we can see has `path` open (scans /proc/*/fd)."""
    try:
        real = os.path.realpath(path)
        pids = [p for p in os.listdir("/proc") if p.isdigit()]
    except OSError:
        return False
    for pid in pids:
        fd_dir = "/proc/%s/fd" % pid
        try:
            fds = os.listdir(fd_dir)
        except OSError:
            continue            # other users' processes, or gone
        for fd in fds:
            try:
                if os.readlink(os.path.join(fd_dir, fd)) == real:
                    return True
            except OSError:
                pass
    return False


def remove_copy(path):
    """Delete a temporary copy and its folder (only inside play_dir())."""
    import shutil
    root = os.path.realpath(play_dir())
    folder = os.path.realpath(os.path.dirname(path))
    if os.path.commonpath([root, folder]) != root or folder == root:
        return False
    shutil.rmtree(folder, ignore_errors=True)
    return True


def clean_play_dir():
    """Remove leftover copies no player has open (Harbor start-up)."""
    root = play_dir()
    try:
        names = os.listdir(root)
    except OSError:
        return
    for n in names:
        d = os.path.join(root, n)
        try:
            files = [os.path.join(d, f) for f in os.listdir(d)]
        except OSError:
            continue
        if not any(open_by_anyone(f) for f in files):
            remove_copy(os.path.join(d, "x"))


def ftp_path(uri):
    """Remote path of an ftp:// file URI ("/DCIM/a b.mp4"), or None."""
    try:
        u = urlsplit(uri or "")
    except ValueError:
        return None
    if u.scheme != "ftp" or not u.path or "\r" in u.path or "\n" in u.path:
        return None
    path = unquote(u.path)
    if "\r" in path or "\n" in path or "\x00" in path:
        return None
    return path


def parse_range(header, size):
    """(start, end) inclusive for a "Range: bytes=..." header.

    None when there is no usable Range (send the whole file); "bad" when
    the range can't be satisfied (416). Only one range is supported.
    """
    if not header:
        return None
    h = header.strip()
    if not h.startswith("bytes=") or "," in h:
        return None
    a, _sep, b = h[6:].strip().partition("-")
    try:
        if a == "":
            n = int(b)
            if n <= 0:
                return "bad"
            return max(0, size - n), size - 1
        start = int(a)
        end = int(b) if b else size - 1
    except ValueError:
        return None
    if start < 0 or start >= size or end < start:
        return "bad"
    return start, min(end, size - 1)


def ftp_open(item, start):
    """Open the file on the phone from byte `start`: (data socket, ftp)."""
    ftp = ftplib.FTP(timeout=FTP_TIMEOUT)
    ftp.encoding = "utf-8"
    ftp.connect(item["host"], item["port"])
    ftp.login(item["user"], item["pin"])
    ftp.voidcmd("TYPE I")
    conn = ftp.transfercmd("RETR " + item["path"], rest=start or None)
    return conn, ftp


def check_login(host, port, user, pin):
    """None when the PIN works, else a short message. Blocking."""
    try:
        ftp = ftplib.FTP(timeout=FTP_TIMEOUT)
        ftp.connect(host, int(port or 21))
        ftp.login(user, pin)
        ftp.close()
        return None
    except ftplib.error_perm:
        return "Wrong PIN"
    except ftplib.error_temp as e:
        return str(e)[4:] or "The phone is busy"
    except (OSError, EOFError, ftplib.Error) as e:
        return "Can't reach the phone (%s)" % (getattr(e, "strerror", None) or e.__class__.__name__)


# ------------------------------------------------------------------ relay

class _Handler(BaseHTTPRequestHandler):
    server_version = "Harbor"
    sys_version = ""
    protocol_version = "HTTP/1.1"

    def log_message(self, *_a):     # never log: the URL holds the token
        pass

    def do_HEAD(self):
        self._serve(body=False)

    def do_GET(self):
        self._serve(body=True)

    def _fail(self, code, extra=None):
        self.send_response(code)
        for k, v in (extra or {}).items():
            self.send_header(k, v)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def _serve(self, body):
        relay = self.server.relay
        token = self.path.split("/")[1] if self.path.startswith("/") else ""
        item = relay.get(token)
        if item is None:
            return self._fail(404)
        size = item["size"]
        rng = parse_range(self.headers.get("Range"), size)
        if rng == "bad":
            return self._fail(416, {"Content-Range": "bytes */%d" % size})
        start, end = rng if rng else (0, size - 1)
        length = end - start + 1
        conn = ftp = None
        if body and length > 0:
            try:
                conn, ftp = relay.opener(item, start)
            except Exception:       # phone gone, wrong PIN...
                return self._fail(502)
        try:
            self.send_response(206 if rng else 200)
            self.send_header("Content-Type", item["ctype"] or "application/octet-stream")
            self.send_header("Accept-Ranges", "bytes")
            self.send_header("Content-Length", str(length))
            if rng:
                self.send_header("Content-Range", "bytes %d-%d/%d" % (start, end, size))
            self.end_headers()
            if conn is None:
                return
            left = length
            while left > 0:
                data = conn.recv(min(CHUNK, left))
                if not data:
                    break
                self.wfile.write(data)
                left -= len(data)
            if left:
                self.close_connection = True
        except (OSError, ValueError):
            self.close_connection = True     # player seeked or closed
        finally:
            for c in (conn, getattr(ftp, "sock", None)):
                try:
                    if c is not None:
                        c.close()
                except OSError:
                    pass


class Relay:
    """Localhost HTTP relay from players to the phone's FTP share."""

    def __init__(self, opener=ftp_open):
        self.opener = opener
        self.items = OrderedDict()
        self.srv = None
        self._lock = threading.Lock()

    def _ensure(self):
        if self.srv is None:
            srv = ThreadingHTTPServer(("127.0.0.1", 0), _Handler)
            srv.daemon_threads = True
            srv.relay = self
            threading.Thread(target=srv.serve_forever, name="harbor-stream", daemon=True).start()
            self.srv = srv
        return self.srv

    @property
    def port(self):
        return self.srv.server_address[1] if self.srv else None

    def add(self, host, port, user, pin, path, size, name, ctype):
        """Register a file; return its http://127.0.0.1 link for the player."""
        item = {"host": host, "port": int(port or 21), "user": user or "anonymous", "pin": pin or "",
                "path": path, "size": int(size), "ctype": ctype or ""}
        token = secrets.token_urlsafe(32)
        with self._lock:
            self.items[token] = item
            while len(self.items) > MAX_ITEMS:
                self.items.popitem(last=False)
        srv = self._ensure()
        safe = quote(os.path.basename(name or "file") or "file", safe="")
        return "http://127.0.0.1:%d/%s/%s" % (srv.server_address[1], token, safe)

    def get(self, token):
        if not token:
            return None
        with self._lock:
            return self.items.get(token)

    def close(self):
        srv, self.srv = self.srv, None
        with self._lock:
            self.items.clear()
        if srv is not None:
            srv.shutdown()
            srv.server_close()


_relay = None


def relay():
    global _relay
    if _relay is None:
        _relay = Relay()
    return _relay
