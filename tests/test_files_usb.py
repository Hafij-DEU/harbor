import os
import queue
import socket
import struct
import threading
import time

from harbor import files_usb as u


def test_pad_never_ends_on_packet_boundary():
    for n in range(0, 300):
        data = b"x" * n
        assert len(u.pad(data)) % 64 != 0


def test_parser_reassembles_split_frames_and_skips_nop():
    raw = u.pad(u.frame(u.DATA, 7, b"hello")) + u.frame(u.NOP, 0) + u.frame(u.CLOSE, 7)
    p = u.Parser()
    got = []
    for i in range(len(raw)):
        got += p.feed(raw[i:i + 1])
    assert got == [(u.DATA, 7, b"hello"), (u.CLOSE, 7, b"")]


def test_parser_rejects_huge_frame():
    p = u.Parser()
    try:
        p.feed(struct.pack(">BII", u.DATA, 1, u.MAX_PAYLOAD + 1))
    except ValueError:
        return
    raise AssertionError("expected ValueError")


def test_bulk_endpoints_from_descriptors():
    dev = bytes([18, 1]) + bytes(16)
    cfg = bytes([9, 2, 32, 0, 1, 1, 0, 0x80, 50])
    iface = bytes([9, 4, 0, 0, 2, 0xFF, 0xFF, 0, 0])
    ep_in = bytes([7, 5, 0x81, 2, 0, 2, 0])
    ep_out = bytes([7, 5, 0x02, 2, 0, 2, 0])
    assert u.bulk_endpoints(dev + cfg + iface + ep_in + ep_out) == (0x81, 0x02)
    assert u.bulk_endpoints(dev + cfg) == (None, None)


def _dev(root, name, vid, pid, ifaces=(), serial="S1", product="Phone"):
    d = os.path.join(root, name)
    os.makedirs(d)
    for k, v in (("idVendor", "%04x" % vid), ("idProduct", "%04x" % pid), ("busnum", "3"),
                 ("devnum", "57"), ("serial", serial), ("product", product), ("manufacturer", "TECNO")):
        with open(os.path.join(d, k), "w") as f:
            f.write(v + "\n")
    for i, (cls, sub, label) in enumerate(ifaces):
        p = os.path.join(d, "%s:1.%d" % (name, i))
        os.makedirs(p)
        for k, v in (("bInterfaceClass", cls), ("bInterfaceSubClass", sub), ("interface", label)):
            with open(os.path.join(p, k), "w") as f:
                f.write(v + "\n")


def test_usb_devices_finds_phones_and_accessories(tmp_path):
    root = str(tmp_path)
    _dev(root, "3-1", 0x0E8D, 0x201C, [("ff", "ff", "MTP")])
    _dev(root, "3-2", 0x046D, 0xC52B, [("03", "01", "")])          # a mouse
    _dev(root, "3-3", u.GOOGLE_VID, 0x2D01, [("ff", "ff", "")], serial="X")
    os.makedirs(os.path.join(root, "usb3"))
    got = {d["sysname"]: d for d in u.usb_devices(root)}
    assert set(got) == {"3-1", "3-3"}
    assert not got["3-1"]["accessory"] and got["3-3"]["accessory"]
    assert got["3-1"]["node"] == "/dev/bus/usb/003/057"
    assert got["3-1"]["name"] == "TECNO Phone"


class Cable:
    """Two in-memory bulk pipes."""

    def __init__(self):
        self.to_phone = queue.Queue()
        self.to_pc = queue.Queue()
        self.writes = []

    def pc_read(self, n):
        try:
            return self.to_pc.get(timeout=0.2)
        except queue.Empty:
            return b""

    def pc_write(self, b):
        self.writes.append(len(b))
        self.to_phone.put(b)
        return len(b)


class FakePhone:
    def __init__(self, cable):
        self.cable = cable
        self.p = u.Parser()
        self.frames = []

    def send(self, kind, sid, payload=b""):
        self.cable.to_pc.put(u.pad(u.frame(kind, sid, payload)))

    def recv_until(self, pred, timeout=5):
        end = time.time() + timeout
        while time.time() < end:
            for f in self.frames:
                if pred(f):
                    return f
            try:
                data = self.cable.to_phone.get(timeout=0.1)
            except queue.Empty:
                continue
            self.frames += self.p.feed(data)
        raise AssertionError("timed out")

    def data_for(self, sid):
        return b"".join(f[2] for f in self.frames if f[0] == u.DATA and f[1] == sid)


def _server(handler):
    srv = socket.socket()
    srv.bind(("127.0.0.1", 0))
    srv.listen(8)

    def loop():
        while True:
            try:
                c, _ = srv.accept()
            except OSError:
                return
            threading.Thread(target=handler, args=(c,), daemon=True).start()
    threading.Thread(target=loop, daemon=True).start()
    return srv


def test_link_hello_and_round_trip():
    def echo(c):
        data = c.recv(100)
        c.sendall(b"got " + data)
        c.close()
    srv = _server(echo)
    cable = Cable()
    link = u.Link(cable.pc_read, cable.pc_write,
                  lambda: socket.create_connection(srv.getsockname(), 5),
                  {"name": "pc", "pin": "123456", "port": 8765})
    link.start()
    phone = FakePhone(cable)
    phone.send(u.HELLO, 0)
    hello = phone.recv_until(lambda f: f[0] == u.HELLO)
    assert b"pin=123456" in hello[2].split(b"\n")
    phone.send(u.OPEN, 5)
    phone.send(u.DATA, 5, b"ping")
    phone.recv_until(lambda f: f[0] == u.CLOSE and f[1] == 5)
    assert phone.data_for(5) == b"got ping"
    assert phone.recv_until(lambda f: f[0] == u.ACK and f[1] == 5)[2] == struct.pack(">I", 4)
    assert all(n % 64 for n in cable.writes)
    link.stop()
    srv.close()


def test_link_respects_credit_so_a_paused_stream_stalls_alone():
    big = b"v" * (u.WINDOW * 3)

    def video(c):
        try:
            c.sendall(big)
        except OSError:
            pass
        c.close()
    srv = _server(video)
    cable = Cable()
    link = u.Link(cable.pc_read, cable.pc_write,
                  lambda: socket.create_connection(srv.getsockname(), 5), {})
    link.start()
    phone = FakePhone(cable)
    phone.send(u.OPEN, 1)
    phone.recv_until(lambda f: f[0] == u.DATA and f[1] == 1)
    time.sleep(0.5)
    while not cable.to_phone.empty():
        phone.frames += phone.p.feed(cable.to_phone.get())
    assert len(phone.data_for(1)) == u.WINDOW          # stops at the window, no ACKs
    phone.send(u.ACK, 1, struct.pack(">I", u.WINDOW * 2))
    phone.send(u.OPEN, 2)                               # another stream still works meanwhile
    phone.recv_until(lambda f: f[0] == u.CLOSE and f[1] == 2 or
                     (f[0] == u.DATA and f[1] == 2))
    end = time.time() + 5
    while len(phone.data_for(1)) < u.WINDOW * 3 and time.time() < end:
        try:
            phone.frames += phone.p.feed(cable.to_phone.get(timeout=0.2))
        except queue.Empty:
            pass
    assert len(phone.data_for(1)) == u.WINDOW * 3
    link.stop()
    srv.close()


def test_open_fails_cleanly_when_server_is_gone():
    cable = Cable()

    def refuse():
        raise ConnectionRefusedError()
    link = u.Link(cable.pc_read, cable.pc_write, refuse, {})
    link.start()
    phone = FakePhone(cable)
    phone.send(u.OPEN, 9)
    phone.recv_until(lambda f: f[0] == u.CLOSE and f[1] == 9)
    link.stop()


def test_phone_close_ends_the_server_connection():
    closed = threading.Event()

    def slow(c):
        try:
            while c.recv(100):
                pass
        except OSError:
            pass
        closed.set()
    srv = _server(slow)
    cable = Cable()
    link = u.Link(cable.pc_read, cable.pc_write,
                  lambda: socket.create_connection(srv.getsockname(), 5), {})
    link.start()
    phone = FakePhone(cable)
    phone.send(u.OPEN, 3)
    time.sleep(0.3)
    phone.send(u.CLOSE, 3)
    assert closed.wait(5)
    link.stop()
    srv.close()


def test_usb_only_share_listens_on_loopback(tmp_path):
    from harbor.files_share import ShareServer
    srv = ShareServer([{"path": str(tmp_path), "upload": False}], port=18765, host="127.0.0.1", minutes=0)
    srv.start()
    try:
        assert srv.httpd.server_address[0] == "127.0.0.1"
    finally:
        srv.stop()


def test_tray_runner_builds_menu_in_every_usb_mode():
    """The tray is GTK 3, so it runs in its own interpreter."""
    import subprocess
    import sys
    code = r"""
from harbor import files_sharetray as t
class S:
    url = qr_text = "http://127.0.0.1:8765/"; pin = "123456"; port = 8765
    host = "127.0.0.1"; running = True; stops_at = None
    def status(self): return {}
class C:
    def register_object(self, *a): return 1
t.AppIndicator = None; t.GUdev = None
for mode in ("ask", "auto", "only", "off"):
    r = t.Runner(S(), C(), mode)
    labels = [label for label, _cb in r._menu_items()]
    assert ("Open on phone over USB cable" in labels) == (mode != "off"), (mode, labels)
    assert ("Copy address" in labels) == (mode != "only"), (mode, labels)
print("ok")
"""
    env = dict(os.environ, PYTHONPATH=os.path.join(os.path.dirname(__file__), "..", "src"))
    r = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True, env=env, timeout=60)
    assert r.stdout.strip().endswith("ok"), r.stderr[-800:]


def test_big_frames_go_out_in_16k_pieces():
    cable = Cable()
    link = u.Link(cable.pc_read, cable.pc_write, lambda: None, {})
    link.send(u.DATA, 1, b"x" * u.MAX_PAYLOAD)
    assert max(cable.writes) <= u.READ_SIZE
    assert all(n == u.READ_SIZE for n in cable.writes[:-1]) and cable.writes[-1] % 64
    p = u.Parser()
    got = []
    while not cable.to_phone.empty():
        got += p.feed(cable.to_phone.get())
    assert got == [(u.DATA, 1, b"x" * u.MAX_PAYLOAD)]


def test_new_phone_session_drops_old_streams():
    def hold(c):
        try:
            while c.recv(100):
                pass
        except OSError:
            pass
    srv = _server(hold)
    cable = Cable()
    link = u.Link(cable.pc_read, cable.pc_write,
                  lambda: socket.create_connection(srv.getsockname(), 5), {"name": "pc"})
    link.start()
    phone = FakePhone(cable)
    phone.send(u.HELLO, 11)
    phone.recv_until(lambda f: f[0] == u.HELLO)
    phone.send(u.OPEN, 1)
    end = time.time() + 5
    while 1 not in link.streams and time.time() < end:
        time.sleep(0.05)
    phone.send(u.HELLO, 11)              # same session: keeps the stream
    time.sleep(0.3)
    assert 1 in link.streams
    phone.send(u.HELLO, 22)              # app restarted
    end = time.time() + 5
    while link.streams and time.time() < end:
        time.sleep(0.05)
    assert not link.streams
    link.stop()
    srv.close()
