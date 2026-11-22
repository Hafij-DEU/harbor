"""Harbor Files: open this PC's shared folders on a phone over a plain USB cable.

No tethering and no developer settings. While Send by Wi-Fi is on, the share
runner can switch a plugged-in Android phone into "USB accessory" mode
(Android Open Accessory). The phone then asks once to open Harbor Files,
and Harbor on the phone reaches the share through the cable.

How the cable link works:
* The PC talks to the phone with plain usbdevfs ioctls (/dev/bus/usb/BBB/DDD,
  no extra library). The desktop's udev rules (uaccess) give the logged-in
  user the phone and the accessory device.
* One bulk pipe each way carries frames: type(1) id(4) len(4) payload.
  The phone sends an empty HELLO once it's listening; the PC answers with
  HELLO ("key=value" lines) holding the PC name and the share PIN. The phone opens a
  stream for each HTTP connection (OPEN); the PC connects it to the share
  server, and DATA / CLOSE pass both ways (CLOSE ends the whole stream, after
  the data sent before it). ACK frames hand back credit, so a
  paused video can't block the cable for every other stream.
* A stream can only reach the share server's own address and port: OPEN has
  no target.
* Every USB write is padded with a no-op frame when its length is a multiple
  of 64, so the other side's read always ends with a short packet.
* While the phone is in accessory mode the PC can't see its files over MTP.
  Stopping the share (or unplugging) resets the phone back to normal.

The PIN only goes down the cable to the phone the user plugged in and agreed
to on its screen. Nothing is logged.
"""

import ctypes
import errno
import os
import queue
import socket
import struct
import threading

GOOGLE_VID = 0x18D1
ACCESSORY_PIDS = (0x2D00, 0x2D01, 0x2D04, 0x2D05)

# The phone's Harbor Files matches these (res/xml/usb_accessory.xml).
MANUFACTURER = "Hafij"
MODEL = "Harbor Files"
DESCRIPTION = "Open this PC's shared folders"
VERSION = "1"
URI = "https://github.com/Hafij-DEU/harbor"
SERIAL = "harbor-pc"

HELLO, OPEN, DATA, CLOSE, ACK, BYE, NOP = 1, 2, 3, 4, 5, 6, 0
HEAD = struct.Struct(">BII")
MAX_PAYLOAD = 64 * 1024
WINDOW = 512 * 1024           # unacknowledged bytes per stream and direction
READ_SIZE = 16384
WRITE_TIMEOUT_MS = 15000

SYSFS = "/sys/bus/usb/devices"

# ------------------------------------------------------------------ usbdevfs


class _Ctrl(ctypes.Structure):
    _fields_ = [("bRequestType", ctypes.c_uint8), ("bRequest", ctypes.c_uint8),
                ("wValue", ctypes.c_uint16), ("wIndex", ctypes.c_uint16),
                ("wLength", ctypes.c_uint16), ("timeout", ctypes.c_uint32),
                ("data", ctypes.c_void_p)]


class _Bulk(ctypes.Structure):
    _fields_ = [("ep", ctypes.c_uint), ("len", ctypes.c_uint),
                ("timeout", ctypes.c_uint), ("data", ctypes.c_void_p)]


def _ioc(direction, nr, size):
    return (direction << 30) | (size << 16) | (ord("U") << 8) | nr


USBDEVFS_CONTROL = _ioc(3, 0, ctypes.sizeof(_Ctrl))
USBDEVFS_BULK = _ioc(3, 2, ctypes.sizeof(_Bulk))
USBDEVFS_CLAIMINTERFACE = _ioc(2, 15, 4)
USBDEVFS_RELEASEINTERFACE = _ioc(2, 16, 4)
USBDEVFS_RESET = _ioc(0, 20, 0)

_libc = None


def _ioctl(fd, req, arg):
    global _libc
    if _libc is None:
        _libc = ctypes.CDLL(None, use_errno=True)
    r = _libc.ioctl(fd, ctypes.c_ulong(req), arg)
    if r < 0:
        e = ctypes.get_errno()
        raise OSError(e, os.strerror(e))
    return r


class UsbDevice:
    """One opened /dev/bus/usb node."""

    def __init__(self, path):
        self.path = path
        self.fd = os.open(path, os.O_RDWR | os.O_CLOEXEC)
        self.claimed = None

    def descriptors(self):
        os.lseek(self.fd, 0, os.SEEK_SET)
        return os.read(self.fd, 4096)

    def control(self, rtype, req, value, index, data=b"", length=0, timeout=2000):
        n = length if rtype & 0x80 else len(data)
        buf = ctypes.create_string_buffer(bytes(data), max(n, 1))
        c = _Ctrl(rtype, req, value, index, n, timeout, ctypes.cast(buf, ctypes.c_void_p))
        got = _ioctl(self.fd, USBDEVFS_CONTROL, ctypes.byref(c))
        return buf.raw[:got]

    def claim(self, iface):
        _ioctl(self.fd, USBDEVFS_CLAIMINTERFACE, ctypes.byref(ctypes.c_uint(iface)))
        self.claimed = iface

    def bulk(self, ep, data=None, size=0, timeout=0):
        """Write `data` to an OUT endpoint, or read up to `size` from an IN one."""
        if data is not None:
            buf = ctypes.create_string_buffer(bytes(data), max(len(data), 1))
            b = _Bulk(ep, len(data), timeout, ctypes.cast(buf, ctypes.c_void_p))
            return _ioctl(self.fd, USBDEVFS_BULK, ctypes.byref(b))
        buf = ctypes.create_string_buffer(size)
        b = _Bulk(ep, size, timeout, ctypes.cast(buf, ctypes.c_void_p))
        got = _ioctl(self.fd, USBDEVFS_BULK, ctypes.byref(b))
        return buf.raw[:got]

    def reset(self):
        try:
            _ioctl(self.fd, USBDEVFS_RESET, None)
        except OSError:
            pass

    def close(self):
        if self.claimed is not None:
            try:
                _ioctl(self.fd, USBDEVFS_RELEASEINTERFACE, ctypes.byref(ctypes.c_uint(self.claimed)))
            except OSError:
                pass
            self.claimed = None
        try:
            os.close(self.fd)
        except OSError:
            pass


def bulk_endpoints(desc, iface=0):
    """(in_ep, out_ep) of the first bulk pair on interface `iface`, from raw descriptors."""
    i, cur, ep_in, ep_out = 0, None, None, None
    while i + 2 <= len(desc):
        ln, kind = desc[i], desc[i + 1]
        if ln < 2:
            break
        if kind == 4 and ln >= 9:                 # interface
            cur = desc[i + 2]
        elif kind == 5 and ln >= 7 and cur == iface:  # endpoint
            addr, attrs = desc[i + 2], desc[i + 3]
            if attrs & 3 == 2:
                if addr & 0x80 and ep_in is None:
                    ep_in = addr
                elif not addr & 0x80 and ep_out is None:
                    ep_out = addr
        i += ln
    return ep_in, ep_out


# ------------------------------------------------------------------ finding phones

def _read(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            return f.read().strip()
    except OSError:
        return ""


def _looks_android(d):
    """MTP, PTP-with-adb or adb interfaces: what an Android phone shows over USB."""
    base = os.path.basename(d)
    try:
        names = os.listdir(d)
    except OSError:
        return False
    for n in names:
        if not n.startswith(base + ":"):
            continue
        p = os.path.join(d, n)
        cls, sub = _read(p + "/bInterfaceClass"), _read(p + "/bInterfaceSubClass")
        label = _read(p + "/interface").upper()
        if (cls == "ff" and sub == "42") or "MTP" in label or (cls == "06" and sub == "01"):
            return True
    return False


def usb_devices(root=SYSFS):
    """Plugged-in Android phones and accessory-mode devices, from sysfs."""
    out = []
    try:
        names = sorted(os.listdir(root))
    except OSError:
        return out
    for n in names:
        if ":" in n or n.startswith("usb"):
            continue
        d = os.path.join(root, n)
        try:
            vid, pid = int(_read(d + "/idVendor") or "0", 16), int(_read(d + "/idProduct") or "0", 16)
            bus, dev = int(_read(d + "/busnum") or "0"), int(_read(d + "/devnum") or "0")
        except ValueError:
            continue
        if not bus or not dev:
            continue
        acc = vid == GOOGLE_VID and pid in ACCESSORY_PIDS
        if not acc and not _looks_android(d):
            continue
        name = (_read(d + "/manufacturer") + " " + _read(d + "/product")).strip() or "Phone"
        out.append({"sysname": n, "vid": vid, "pid": pid, "accessory": acc, "name": name,
                    "serial": _read(d + "/serial"),
                    "node": "/dev/bus/usb/%03d/%03d" % (bus, dev)})
    return out


def switch_to_accessory(node):
    """Ask the phone to come back as a Harbor accessory. Raises OSError."""
    d = UsbDevice(node)
    try:
        ver = d.control(0xC0, 51, 0, 0, length=2)
        if len(ver) < 2 or struct.unpack("<H", ver)[0] < 1:
            raise OSError(errno.ENOTSUP, "This phone doesn't support USB accessories")
        for i, s in enumerate((MANUFACTURER, MODEL, DESCRIPTION, VERSION, URI, SERIAL)):
            d.control(0x40, 52, 0, i, s.encode() + b"\0")
        d.control(0x40, 53, 0, 0)
    finally:
        d.close()


# ------------------------------------------------------------------ frames

def frame(kind, sid, payload=b""):
    return HEAD.pack(kind, sid, len(payload)) + payload


def pad(data):
    """Never end a USB write on a packet boundary (no zero-length packets needed)."""
    return data + frame(NOP, 0) if len(data) % 64 == 0 else data


class Parser:
    """Turns USB reads back into (kind, id, payload) frames."""

    def __init__(self):
        self.buf = bytearray()

    def feed(self, data):
        self.buf += data
        out = []
        while len(self.buf) >= HEAD.size:
            kind, sid, ln = HEAD.unpack_from(self.buf)
            if ln > MAX_PAYLOAD:
                raise ValueError("frame too big")
            if len(self.buf) < HEAD.size + ln:
                break
            payload = bytes(self.buf[HEAD.size:HEAD.size + ln])
            del self.buf[:HEAD.size + ln]
            if kind != NOP:
                out.append((kind, sid, payload))
        return out


def hello_bytes(info):
    """HELLO payload: "key=value" lines (no newlines inside values)."""
    lines = ("%s=%s" % (k, str(v).replace("\n", " ").replace("\r", " ")) for k, v in info.items())
    return "\n".join(lines).encode()


# ------------------------------------------------------------------ tunnel

class _Stream:
    def __init__(self, sid, sock):
        self.sid = sid
        self.sock = sock
        self.credit = WINDOW
        self.cv = threading.Condition()
        self.inbox = queue.Queue()
        self.closed = False


class Link:
    """The PC end of the cable: phone streams to the share server.

    `read(size)` / `write(data)` move raw bytes over the cable (bulk transfers
    on a real phone; anything in tests). `connect()` opens a socket to the
    share server for each stream.
    """

    def __init__(self, read, write, connect, hello, on_end=None):
        self._read = read
        self._write = write
        self.connect = connect
        self.hello = hello
        self.on_end = on_end
        self.streams = {}
        self.lock = threading.Lock()
        self.wlock = threading.Lock()
        self.alive = False

    def send(self, kind, sid, payload=b""):
        with self.wlock:
            self._write(pad(frame(kind, sid, payload)))

    def start(self):
        self.alive = True
        # HELLO goes out when the phone asks for it: bytes sent before the
        # phone opens the accessory are lost.
        threading.Thread(target=self._reader, name="harbor-usb-read", daemon=True).start()

    def stop(self):
        if not self.alive:
            return
        self.alive = False
        try:
            self.send(BYE, 0)
        except OSError:
            pass
        with self.lock:
            streams = list(self.streams.values())
            self.streams.clear()
        for s in streams:
            self._drop(s)

    def _reader(self):
        parser = Parser()
        try:
            while self.alive:
                data = self._read(READ_SIZE)
                if not data:
                    continue
                for kind, sid, payload in parser.feed(data):
                    self._on_frame(kind, sid, payload)
        except (OSError, ValueError):
            pass
        finally:
            was = self.alive
            self.stop()
            if was and self.on_end:
                self.on_end()

    def _on_frame(self, kind, sid, payload):
        if kind == OPEN:
            # Registered now, so DATA sent right after OPEN waits in its inbox.
            s = _Stream(sid, None)
            with self.lock:
                if sid in self.streams:
                    return
                self.streams[sid] = s
            threading.Thread(target=self._open, args=(s,), daemon=True).start()
            return
        if kind == BYE:
            self.alive = False
            return
        if kind == HELLO:
            self._safe_send(HELLO, 0, hello_bytes(self.hello))
            return
        with self.lock:
            s = self.streams.get(sid)
        if s is None:
            return
        if kind == DATA:
            s.inbox.put(payload)
        elif kind == CLOSE:
            s.inbox.put(None)
        elif kind == ACK and len(payload) == 4:
            with s.cv:
                s.credit += struct.unpack(">I", payload)[0]
                s.cv.notify_all()

    def _open(self, s):
        try:
            sock = self.connect()
        except OSError:
            self._close(s)
            return
        with s.cv:
            s.sock = sock
            dead = s.closed
        if dead:
            sock.close()
            return
        threading.Thread(target=self._to_server, args=(s,), daemon=True).start()
        self._to_phone(s)

    def _to_server(self, s):
        """Phone bytes into the server socket, handing credit back as they go."""
        try:
            while True:
                data = s.inbox.get()
                if data is None:
                    break
                s.sock.sendall(data)
                self._safe_send(ACK, s.sid, struct.pack(">I", len(data)))
        except OSError:
            pass
        self._close(s)      # CLOSE from the phone ends the whole stream

    def _to_phone(self, s):
        """Server bytes to the phone, never more than the phone's credit."""
        try:
            while self.alive:
                with s.cv:
                    while s.credit <= 0 and self.alive and not s.closed:
                        s.cv.wait(1)
                    if s.closed or not self.alive:
                        break
                    n = min(s.credit, MAX_PAYLOAD)
                data = s.sock.recv(n)
                if not data:
                    break
                with s.cv:
                    s.credit -= len(data)
                self.send(DATA, s.sid, data)
        except OSError:
            pass
        self._close(s)

    def _safe_send(self, kind, sid, payload=b""):
        try:
            self.send(kind, sid, payload)
        except OSError:
            pass

    def _close(self, s):
        with self.lock:
            known = self.streams.pop(s.sid, None) is not None
        if known:
            self._safe_send(CLOSE, s.sid)
        self._drop(s)

    @staticmethod
    def _drop(s):
        with s.cv:
            s.closed = True
            s.cv.notify_all()
        s.inbox.put(None)
        try:
            if s.sock is not None:
                s.sock.close()
        except OSError:
            pass


class PhoneLink:
    """A phone in accessory mode, tunnelled to the share server at host:port."""

    def __init__(self, node, host, port, hello, on_end=None):
        self.dev = UsbDevice(node)
        self.node = node
        try:
            ep_in, ep_out = bulk_endpoints(self.dev.descriptors())
            if ep_in is None or ep_out is None:
                raise OSError(errno.ENODEV, "No accessory endpoints")
            self.dev.claim(0)
        except OSError:
            self.dev.close()
            raise
        dev = self.dev
        self.link = Link(
            read=lambda n: dev.bulk(ep_in, size=n, timeout=0),
            write=lambda b: dev.bulk(ep_out, data=b, timeout=WRITE_TIMEOUT_MS),
            connect=lambda: socket.create_connection((host, port), 10),
            hello=hello, on_end=on_end)

    def start(self):
        self.link.start()

    def stop(self, reset=True):
        """End the link; reset=True puts the phone back in normal (MTP) mode."""
        self.link.stop()
        if reset:
            self.dev.reset()
        self.dev.close()
