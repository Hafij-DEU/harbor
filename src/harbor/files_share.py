"""Harbor Files: share folders over Wi-Fi (web page + minimal WebDAV).

Security rules (keep them when you change this file):
  * listens on the LAN address only, never 0.0.0.0, and never starts by itself
  * a 6-digit PIN opens a session; the session is a random 128-bit token in an
    HttpOnly SameSite=Strict cookie; every compare uses hmac.compare_digest
  * 5 wrong PINs lock everyone out for 5 minutes
  * every path is resolved with realpath and must stay inside a shared folder;
    hidden names (".x") are never listed or served
  * read-only unless the folder allows uploads; uploads never overwrite and are
    saved 0644; WebDAV DELETE moves to the Trash
  * html/svg/xml/js are served as downloads, never rendered; CSP, nosniff and
    no-referrer headers on every reply
  * WebDAV under /dav/ uses Basic auth: user "harbor", password = PIN
  * the PIN and the token are never logged (log_message is silenced)
"""

import base64
import hmac
import html
import ipaddress
import json
import mimetypes
import os
import secrets
import socket
import threading
import time
from email.utils import formatdate
from http.cookies import SimpleCookie
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, quote, unquote, urlsplit

LOCK_FAILS = 5
LOCK_SECONDS = 300
DAV_USER = "harbor"
CHUNK = 64 * 1024
COOKIE = "harbor_share"
INLINE_TYPES = ("image/png", "image/jpeg", "image/gif", "image/webp", "image/avif",
                "image/bmp", "application/pdf", "text/plain")
RISKY_EXTS = {".html", ".htm", ".xhtml", ".shtml", ".svg", ".svgz", ".xml", ".xsl",
              ".js", ".mjs", ".css"}
CSP = ("default-src 'none'; style-src 'unsafe-inline'; script-src 'self'; img-src 'self'; "
       "media-src 'self'; connect-src 'self'; form-action 'self'; frame-ancestors 'none'; "
       "base-uri 'none'")


# ------------------------------------------------------------------ helpers

def lan_address(prefer=""):
    """This computer's private LAN IPv4 address, or None.

    Uses the default route (a UDP connect sends nothing). Only private and
    link-local addresses qualify, so a public IP is never used."""
    if prefer:
        try:
            ip = ipaddress.ip_address(prefer)
            if ip.is_private and not ip.is_loopback:
                return str(ip)
        except ValueError:
            pass
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
            s.connect(("192.0.2.1", 9))
            addr = s.getsockname()[0]
    except OSError:
        return None
    ip = ipaddress.ip_address(addr)
    if ip.is_private and not ip.is_loopback:
        return addr
    return None


def new_pin():
    return "%06d" % secrets.randbelow(1000000)


def safe_name(name):
    """A plain file name an uploader may use, or None."""
    name = (name or "").strip()
    if (not name or name in (".", "..") or name.startswith(".") or "/" in name
            or "\\" in name or "\x00" in name or len(name.encode("utf-8")) > 255):
        return None
    return name


def unique_name(folder, name):
    """name, or "name (2).ext", "name (3).ext"... whichever is free."""
    stem, ext = os.path.splitext(name)
    cand, n = name, 1
    while os.path.lexists(os.path.join(folder, cand)):
        n += 1
        cand = "%s (%d)%s" % (stem, n, ext)
    return cand


def share_names(paths):
    """{url_name: real path} with unique names (folder base names)."""
    out = {}
    for p in paths:
        base = os.path.basename(os.path.normpath(p)) or "Files"
        base = base.lstrip(".") or "Files"
        name, n = base, 1
        while name in out:
            n += 1
            name = "%s (%d)" % (base, n)
        out[name] = os.path.realpath(p)
    return out


def is_inside(path, root):
    return path == root or path.startswith(root.rstrip(os.sep) + os.sep)


def content_type(path):
    t = mimetypes.guess_type(path)[0] or "application/octet-stream"
    return t


def disposition(path, force_download=False):
    """Content-Disposition: risky types are always downloads."""
    name = os.path.basename(path)
    t = content_type(path)
    ext = os.path.splitext(path)[1].lower()
    inline = (not force_download and ext not in RISKY_EXTS
              and (t in INLINE_TYPES or t.startswith(("video/", "audio/"))))
    return "%s; filename*=UTF-8''%s" % ("inline" if inline else "attachment",
                                        quote(name, safe="")), inline


def human_size(n):
    for unit in ("B", "KB", "MB", "GB"):
        if n < 1024 or unit == "GB":
            return ("%d %s" if unit == "B" else "%.1f %s") % (n, unit)
        n /= 1024


class Guard:
    """PIN check with a shared lockout after LOCK_FAILS wrong tries."""

    def __init__(self, secret, now=time.monotonic):
        self.secret = secret
        self.now = now
        self.fails = 0
        self.locked_until = 0.0
        self.lock = threading.Lock()

    def check(self, given):
        """"ok", "bad" or "locked"."""
        with self.lock:
            if self.now() < self.locked_until:
                return "locked"
            if hmac.compare_digest(str(given or "").encode(), self.secret.encode()):
                self.fails = 0
                return "ok"
            self.fails += 1
            if self.fails >= LOCK_FAILS:
                self.fails = 0
                self.locked_until = self.now() + LOCK_SECONDS
            return "bad"


# ------------------------------------------------------------------ server

# The share runs in its own process (files_sharetray) under this session-bus name.
BUS_NAME = "io.github.hafij_deu.Harbor.Share"
OBJECT_PATH = "/io/github/hafij_deu/Harbor/Share"


class ShareServer:
    """folders: [{"path": str, "upload": bool}]. start() -> url."""

    def __init__(self, folders, port=8765, upload_max_mb=4096, host=None, minutes=30,
                 on_stop=None):
        self.roots = share_names([f["path"] for f in folders])
        uploads = {os.path.realpath(f["path"]) for f in folders if f.get("upload")}
        self.upload = {n: (p in uploads) for n, p in self.roots.items()}
        self.port = port
        self.upload_max = upload_max_mb * 1024 * 1024
        self.host = host
        self.minutes = minutes
        self.on_stop = on_stop
        self.pin = new_pin()
        self.token = secrets.token_hex(16)
        self.guard = Guard(self.pin)
        self.httpd = None
        self.thread = None
        self.timer = None
        self.stops_at = None
        self.requests = 0

    @property
    def running(self):
        return self.httpd is not None

    @property
    def url(self):
        return "http://%s:%d/" % (self.host, self.port)

    @property
    def qr_text(self):
        """What the QR code says: the address plus the PIN, so a scan signs in."""
        return "%s#pin=%s" % (self.url, self.pin)

    def status(self):
        return {"url": self.url, "dav_url": self.dav_url, "qr_text": self.qr_text,
                "pin": self.pin, "host": self.host, "port": self.port,
                "stops_at": self.stops_at}

    @property
    def dav_url(self):
        return "dav://%s@%s:%d/dav/" % (DAV_USER, self.host, self.port)

    def start(self):
        if self.running:
            return self.url
        self.host = self.host or lan_address()
        if not self.host:
            raise OSError("No Wi-Fi or local network connection")
        for f in self.roots.values():
            if not os.path.isdir(f):
                raise OSError("Folder not found: %s" % f)
        handler = type("H", (Handler,), {"share": self})
        last = None
        for port in range(self.port, self.port + 10):
            try:
                self.httpd = ThreadingHTTPServer((self.host, port), handler)
                self.port = port
                break
            except OSError as e:
                last = e
        else:
            raise OSError("No free port: %s" % last)
        self.httpd.daemon_threads = True
        self.thread = threading.Thread(target=self.httpd.serve_forever, daemon=True,
                                       name="harbor-share")
        self.thread.start()
        if self.minutes:
            self.stops_at = time.time() + self.minutes * 60
            self.timer = threading.Timer(self.minutes * 60, self.stop)
            self.timer.daemon = True
            self.timer.start()
        return self.url

    def stop(self):
        httpd, self.httpd = self.httpd, None
        if self.timer is not None:
            self.timer.cancel()
            self.timer = None
        if httpd is None:
            return
        httpd.shutdown()
        httpd.server_close()
        self.stops_at = None
        if self.on_stop:
            self.on_stop()

    # paths ---------------------------------------------------------
    def resolve(self, parts):
        """(share name, real path) for url parts, or (None, None)."""
        if not parts or parts[0] not in self.roots:
            return None, None
        for p in parts[1:]:
            if not p or p in (".", "..") or p.startswith(".") or "\x00" in p or "/" in p:
                return None, None
        root = self.roots[parts[0]]
        path = os.path.realpath(os.path.join(root, *parts[1:]))
        if not is_inside(path, root):
            return None, None
        return parts[0], path


def split_path(urlpath):
    """"/f/Music/a%20b/" -> ["Music", "a b"] after the prefix; keeps order."""
    return [unquote(p) for p in urlpath.split("/") if p]


JS = r"""
document.addEventListener('DOMContentLoaded', () => {
  // QR code from Harbor: http://ip:port/#pin=123456 signs in with no typing.
  const m = /(?:^#|&)pin=(\d{6})(?:&|$)/.exec(location.hash);
  if (m) {
    history.replaceState(null, '', location.pathname + location.search);
    const p = document.querySelector('input[name=pin]');
    if (p) { p.value = m[1]; p.form.submit(); return; }
  }
  const input = document.getElementById('files');
  if (!input) return;
  const status = document.getElementById('status');
  input.addEventListener('change', async () => {
    let done = 0;
    for (const f of input.files) {
      status.textContent = 'Uploading ' + f.name + '…';
      const r = await fetch(location.pathname + encodeURIComponent(f.name),
        {method: 'PUT', body: f, headers: {'X-Harbor-Upload': '1'}});
      if (r.ok) done++;
      else status.textContent = f.name + ': ' + (await r.text());
    }
    if (done === input.files.length) location.reload();
  });
});
"""

STYLE = """
body{font:15px system-ui,sans-serif;margin:0;background:#f5f5f7;color:#1d1d1f}
@media (prefers-color-scheme:dark){body{background:#1c1c1e;color:#f2f2f7}
 .row{border-color:#333!important}a{color:#6cb4ff}}
header{padding:14px 16px;font-weight:600;font-size:17px}
main{max-width:820px;margin:0 auto;padding:0 12px 40px}
.row{display:flex;gap:10px;padding:11px 6px;border-bottom:1px solid #ddd;align-items:center}
.row a{flex:1;text-decoration:none;word-break:break-all}.s{opacity:.6;font-size:13px}
.box{max-width:340px;margin:12vh auto;text-align:center}
input[type=password],input[type=text]{font-size:22px;letter-spacing:6px;text-align:center;
 width:200px;padding:8px}button,label.b{font-size:16px;padding:8px 18px;margin:10px}
.err{color:#d33}
"""


def page(title, body):
    return ("<!doctype html><html><head><meta charset=utf-8>"
            "<meta name=viewport content='width=device-width,initial-scale=1'>"
            "<meta name=referrer content=no-referrer><title>%s</title><style>%s</style>"
            "<script src='/static/app.js'></script></head><body>%s</body></html>"
            % (html.escape(title), STYLE, body)).encode("utf-8")


class Handler(BaseHTTPRequestHandler):
    share = None                 # set on the subclass
    server_version = "Harbor"
    sys_version = ""
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):          # never log (URLs may carry names)
        pass

    # replies ------------------------------------------------------
    def _headers(self, code, ctype, length, extra=()):
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(length))
        self.send_header("Content-Security-Policy", CSP)
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Referrer-Policy", "no-referrer")
        self.send_header("X-Frame-Options", "DENY")
        self.send_header("Cache-Control", "no-store")
        for k, v in extra:
            self.send_header(k, v)
        self.end_headers()

    def _send(self, code, body=b"", ctype="text/plain; charset=utf-8", extra=(), close=False):
        if isinstance(body, str):
            body = body.encode("utf-8")
        if close:
            self.close_connection = True
            extra = tuple(extra) + (("Connection", "close"),)
        self._headers(code, ctype, len(body), extra)
        if self.command != "HEAD":
            self.wfile.write(body)

    def _html(self, code, title, body, extra=()):
        self._send(code, page(title, body), "text/html; charset=utf-8", extra)

    def _length(self):
        """Content-Length (0 when absent), None when invalid or chunked."""
        if self.headers.get("Transfer-Encoding"):
            return None
        try:
            n = int(self.headers.get("Content-Length") or 0)
        except ValueError:
            return None
        return n if n >= 0 else None

    # auth ---------------------------------------------------------
    def _cookie_ok(self):
        c = SimpleCookie()
        try:
            c.load(self.headers.get("Cookie", ""))
        except Exception:
            return False
        m = c.get(COOKIE)
        return m is not None and hmac.compare_digest(m.value.encode(),
                                                     self.share.token.encode())

    def _origin_ok(self):
        o = self.headers.get("Origin")
        if not o:
            return True
        return urlsplit(o).netloc == self.headers.get("Host", "")

    def _dav_auth(self):
        """True when Basic auth is right; otherwise replies 401/429."""
        h = self.headers.get("Authorization", "")
        if h.startswith("Basic "):
            try:
                user, _, pw = base64.b64decode(h[6:], validate=True).decode().partition(":")
            except (ValueError, UnicodeDecodeError):
                user = pw = ""
            if hmac.compare_digest(user.encode(), DAV_USER.encode()):
                r = self.share.guard.check(pw)
                if r == "ok":
                    return True
                if r == "locked":
                    self._send(429, "Too many wrong PINs. Try again in 5 minutes.", close=True)
                    return False
            else:
                self.share.guard.check(None)
        self._send(401, "PIN needed", extra=(("WWW-Authenticate",
                                              'Basic realm="Harbor", charset="UTF-8"'),),
                   close=True)
        return False

    # dispatch -----------------------------------------------------
    def _route(self):
        self.share.requests += 1
        path = urlsplit(self.path).path
        if path.startswith("/dav"):
            if not self._dav_auth():
                return
            parts = split_path(path)[1:]
            m = getattr(self, "dav_" + self.command.lower(), None)
            if m is None:
                return self._send(405, "Not supported", close=True)
            return m(parts, path)
        if path == "/static/app.js" and self.command in ("GET", "HEAD"):
            return self._send(200, JS, "text/javascript; charset=utf-8")
        if path == "/login":
            return self.login()
        if path == "/logout":
            return self._send(303, "", extra=(("Location", "/login"), (
                "Set-Cookie", "%s=; Max-Age=0; Path=/; HttpOnly; SameSite=Strict" % COOKIE)))
        if not self._cookie_ok():
            if self.command in ("GET", "HEAD"):
                return self._send(303, "", extra=(("Location", "/login"),))
            return self._send(401, "Sign in first", close=True)
        if path == "/" and self.command in ("GET", "HEAD"):
            return self.index()
        if path.startswith("/f/"):
            parts = split_path(path)[1:]
            if self.command in ("GET", "HEAD"):
                return self.web_get(parts, path)
            if self.command == "PUT":
                if not self._origin_ok() or self.headers.get("X-Harbor-Upload") != "1":
                    return self._send(403, "Refused", close=True)
                return self.put(parts)
        self._send(404, "Not found", close=True)

    def _safe(self):
        try:
            self._route()
        except (BrokenPipeError, ConnectionResetError):
            pass
        except Exception:               # never leak a traceback to the client
            try:
                self._send(500, "Error", close=True)
            except OSError:
                pass

    do_GET = do_HEAD = do_POST = do_PUT = do_OPTIONS = do_PROPFIND = do_MKCOL = _safe
    do_DELETE = do_MOVE = do_COPY = do_LOCK = do_UNLOCK = do_PROPPATCH = _safe

    # web ----------------------------------------------------------
    def login(self):
        if self.command == "POST":
            n = self._length()
            if n is None or n > 200:
                return self._send(413, "Too large", close=True)
            pin = parse_qs(self.rfile.read(n).decode("utf-8", "replace")).get("pin", [""])[0]
            r = self.share.guard.check(pin.strip())
            if r == "ok":
                return self._send(303, "", extra=(("Location", "/"), (
                    "Set-Cookie", "%s=%s; Path=/; HttpOnly; SameSite=Strict"
                    % (COOKIE, self.share.token))))
            msg = ("Too many wrong PINs. Try again in 5 minutes." if r == "locked"
                   else "Wrong PIN")
            return self._login_form(msg, 429 if r == "locked" else 403)
        return self._login_form("", 200)

    def _login_form(self, msg, code):
        body = ("<div class=box><h2>Harbor Files</h2><p>Enter the PIN shown on the computer.</p>"
                "<form method=post action=/login><input type=password name=pin "
                "inputmode=numeric autocomplete=off maxlength=6 autofocus><br>"
                "<button>Open</button></form><p class=err>%s</p></div>" % html.escape(msg))
        self._html(code, "Harbor Files", body)

    def index(self):
        rows = "".join("<div class=row>📁 <a href='/f/%s/'>%s</a><span class=s>%s</span></div>"
                       % (quote(n), html.escape(n),
                          "uploads on" if self.share.upload[n] else "read-only")
                       for n in self.share.roots)
        self._html(200, "Harbor Files", "<header>Shared from this computer</header><main>%s"
                   "<p><a href=/logout>Sign out</a></p></main>" % rows)

    def web_get(self, parts, urlpath):
        name, path = self.share.resolve(parts)
        if path is None or not os.path.exists(path):
            return self._send(404, "Not found", close=True)
        if os.path.isdir(path):
            if not urlpath.endswith("/"):
                return self._send(301, "", extra=(("Location", quote(urlpath) + "/"),))
            return self.listing(name, parts, path)
        q = parse_qs(urlsplit(self.path).query)
        self.send_file(path, force_download="dl" in q)

    def listing(self, name, parts, path):
        try:
            entries = sorted(os.scandir(path),
                             key=lambda e: (not e.is_dir(), e.name.casefold()))
        except OSError:
            return self._send(403, "Can't read this folder", close=True)
        rows = []
        if len(parts) > 1:
            rows.append("<div class=row>⬆️ <a href='../'>Up</a></div>")
        for e in entries:
            if e.name.startswith("."):
                continue
            try:
                if not is_inside(os.path.realpath(e.path), self.share.roots[name]):
                    continue
                d = e.is_dir()
                size = "" if d else human_size(e.stat().st_size)
            except OSError:
                continue
            href = quote(e.name) + ("/" if d else "")
            dl = "" if d else " <a class=s href='%s?dl=1'>download</a>" % href
            rows.append("<div class=row>%s <a href='%s'>%s</a><span class=s>%s</span>%s</div>"
                        % ("📁" if d else "📄", href, html.escape(e.name), size, dl))
        up = ""
        if self.share.upload[name]:
            up = ("<p><label class=b>⬆️ Upload files<input id=files type=file multiple hidden>"
                  "</label> <span id=status class=s></span></p>")
        title = " / ".join(parts)
        self._html(200, title, "<header><a href='/'>Harbor</a> / %s</header><main>%s%s</main>"
                   % (html.escape(title), up, "".join(rows) or "<p class=s>Empty folder</p>"))

    def send_file(self, path, force_download=False):
        try:
            f = open(path, "rb")
        except OSError:
            return self._send(403, "Can't read this file", close=True)
        with f:
            size = os.fstat(f.fileno()).st_size
            disp, inline = disposition(path, force_download)
            ctype = content_type(path) if inline else "application/octet-stream"
            if ctype == "text/plain":
                ctype += "; charset=utf-8"
            start, end, code = 0, size - 1, 200
            rng = self.headers.get("Range", "")
            extra = [("Content-Disposition", disp), ("Accept-Ranges", "bytes"),
                     ("Last-Modified", formatdate(os.path.getmtime(path), usegmt=True))]
            if rng.startswith("bytes=") and "," not in rng and size:
                a, _, b = rng[6:].partition("-")
                try:
                    if a:
                        start, end = int(a), min(int(b) if b else size - 1, size - 1)
                    else:
                        start, end = max(0, size - int(b)), size - 1
                except ValueError:
                    start, end = 0, size - 1
                if start > end or start >= size:
                    return self._send(416, "", extra=(("Content-Range", "bytes */%d" % size),))
                code = 206
                extra.append(("Content-Range", "bytes %d-%d/%d" % (start, end, size)))
            length = max(0, end - start + 1)
            self._headers(code, ctype, length, extra)
            if self.command == "HEAD":
                return
            f.seek(start)
            left = length
            while left > 0:
                buf = f.read(min(CHUNK, left))
                if not buf:
                    break
                self.wfile.write(buf)
                left -= len(buf)

    # uploads ------------------------------------------------------
    def put(self, parts):
        """Save the request body as parts[-1] in its folder. Never overwrites:
        a taken name becomes "name (2).ext"."""
        if len(parts) < 2:
            return self._send(403, "Refused", close=True)
        fname = safe_name(parts[-1])
        name, folder = self.share.resolve(parts[:-1])
        if fname is None or folder is None or not os.path.isdir(folder):
            return self._send(400, "Bad file name", close=True)
        if not self.share.upload[name]:
            return self._send(403, "This folder is read-only", close=True)
        length = self._length()
        if length is None or "Content-Length" not in self.headers:
            return self._send(411, "Length required", close=True)
        if length > self.share.upload_max:
            return self._send(413, "File too large", close=True)
        tmp = os.path.join(folder, ".harbor-upload-%s.part" % secrets.token_hex(6))
        try:
            fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
        except OSError:
            return self._send(403, "Can't write here", close=True)
        ok = False
        try:
            with os.fdopen(fd, "wb") as out:
                left = length
                while left > 0:
                    buf = self.rfile.read(min(CHUNK, left))
                    if not buf:
                        break
                    out.write(buf)
                    left -= len(buf)
            if left:
                return self._send(400, "Upload cut short", close=True)
            os.chmod(tmp, 0o644)
            final = fname
            for _ in range(1000):
                final = unique_name(folder, final)
                try:
                    os.close(os.open(os.path.join(folder, final),
                                     os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644))
                    break
                except FileExistsError:
                    continue
            os.replace(tmp, os.path.join(folder, final))
            ok = True
            self._send(201, json.dumps({"name": final}), "application/json")
        finally:
            if not ok and os.path.exists(tmp):
                os.unlink(tmp)

    # webdav -------------------------------------------------------
    def _dav_target(self, parts):
        if not parts:
            return None, None
        return self.share.resolve(parts)

    def dav_options(self, parts, urlpath):
        self._send(200, "", extra=(("DAV", "1"), (
            "Allow", "OPTIONS, GET, HEAD, PROPFIND, PUT, MKCOL, DELETE")))

    def dav_get(self, parts, urlpath):
        name, path = self._dav_target(parts)
        if path is None or not os.path.isfile(path):
            return self._send(404 if path is None or not os.path.exists(path) else 405,
                              "Not a file", close=True)
        self.send_file(path, force_download=True)

    dav_head = dav_get

    def dav_put(self, parts, urlpath):
        name, path = self._dav_target(parts)
        if path is not None and os.path.lexists(path):
            return self._send(412, "Harbor never overwrites files", close=True)
        self.put(parts)

    def dav_mkcol(self, parts, urlpath):
        if len(parts) < 2 or safe_name(parts[-1]) is None:
            return self._send(403, "Refused", close=True)
        name, parent = self.share.resolve(parts[:-1])
        if parent is None or not os.path.isdir(parent):
            return self._send(409, "No parent folder", close=True)
        if not self.share.upload[name]:
            return self._send(403, "This folder is read-only", close=True)
        try:
            os.mkdir(os.path.join(parent, parts[-1]), 0o755)
        except FileExistsError:
            return self._send(405, "Exists", close=True)
        except OSError:
            return self._send(403, "Can't create", close=True)
        self._send(201, "")

    def dav_delete(self, parts, urlpath):
        name, path = self._dav_target(parts)
        if path is None or len(parts) < 2 or not os.path.lexists(path):
            return self._send(404, "Not found", close=True)
        if not self.share.upload[name]:
            return self._send(403, "This folder is read-only", close=True)
        from gi.repository import Gio, GLib
        try:
            Gio.File.new_for_path(path).trash(None)
        except GLib.Error:
            return self._send(403, "Can't move to the Trash", close=True)
        self._send(204, "")

    def dav_propfind(self, parts, urlpath):
        n = self._length()
        if n is None or n > 65536:
            return self._send(413, "Too large", close=True)
        self.rfile.read(n)
        depth = self.headers.get("Depth", "1")
        out = []
        if not parts:
            out.append(self._prop("/dav/", "Harbor", None))
            if depth != "0":
                for nm, p in self.share.roots.items():
                    out.append(self._prop("/dav/%s/" % quote(nm), nm, p))
        else:
            name, path = self._dav_target(parts)
            if path is None or not os.path.exists(path):
                return self._send(404, "Not found", close=True)
            base = "/dav/" + "/".join(quote(p) for p in parts)
            isdir = os.path.isdir(path)
            out.append(self._prop(base + ("/" if isdir else ""), parts[-1], path))
            if isdir and depth != "0":
                try:
                    for e in os.scandir(path):
                        if e.name.startswith("."):
                            continue
                        real = os.path.realpath(e.path)
                        if not is_inside(real, self.share.roots[name]):
                            continue
                        out.append(self._prop(base + "/" + quote(e.name)
                                              + ("/" if e.is_dir() else ""), e.name, real))
                except OSError:
                    pass
        body = ('<?xml version="1.0" encoding="utf-8"?>\n<D:multistatus xmlns:D="DAV:">%s'
                "</D:multistatus>" % "".join(out))
        self._send(207, body, "application/xml; charset=utf-8")

    def _prop(self, href, display, path):
        props = ["<D:displayname>%s</D:displayname>" % html.escape(display)]
        if path is None or os.path.isdir(path):
            props.append("<D:resourcetype><D:collection/></D:resourcetype>")
        else:
            props.append("<D:resourcetype/>")
            try:
                props.append("<D:getcontentlength>%d</D:getcontentlength>"
                             % os.path.getsize(path))
            except OSError:
                pass
            props.append("<D:getcontenttype>%s</D:getcontenttype>"
                         % html.escape(content_type(path)))
        if path is not None:
            try:
                props.append("<D:getlastmodified>%s</D:getlastmodified>"
                             % formatdate(os.path.getmtime(path), usegmt=True))
            except OSError:
                pass
        return ("<D:response><D:href>%s</D:href><D:propstat><D:prop>%s</D:prop>"
                "<D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>"
                % (html.escape(href), "".join(props)))
