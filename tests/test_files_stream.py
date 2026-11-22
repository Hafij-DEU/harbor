"""Streaming big phone media through the localhost relay."""
import io
import urllib.error
import urllib.request

import pytest

from harbor import files_stream as fs

DATA = bytes(range(256)) * 400          # 102400 bytes


class FakeConn:
    def __init__(self, start):
        self.buf = io.BytesIO(DATA[start:])
        self.closed = False

    def recv(self, n):
        return self.buf.read(n)

    def close(self):
        self.closed = True


class FakeFtp:
    sock = None


@pytest.fixture
def relay():
    seen = []

    def opener(item, start):
        seen.append((item["path"], start, item["pin"]))
        return FakeConn(start), FakeFtp()
    r = fs.Relay(opener)
    r.seen = seen
    yield r
    r.close()


def get(url, rng=None, method="GET"):
    req = urllib.request.Request(url, method=method)
    if rng:
        req.add_header("Range", rng)
    with urllib.request.urlopen(req, timeout=5) as resp:
        return resp.status, dict(resp.headers), resp.read()


def test_parse_range():
    assert fs.parse_range(None, 100) is None
    assert fs.parse_range("bytes=0-", 100) == (0, 99)
    assert fs.parse_range("bytes=10-19", 100) == (10, 19)
    assert fs.parse_range("bytes=90-500", 100) == (90, 99)
    assert fs.parse_range("bytes=-10", 100) == (90, 99)
    assert fs.parse_range("bytes=100-", 100) == "bad"
    assert fs.parse_range("bytes=5-2", 100) == "bad"
    assert fs.parse_range("bytes=0-1,5-6", 100) is None     # multi-range: whole file
    assert fs.parse_range("items=0-1", 100) is None


def test_ftp_path_and_choice():
    assert fs.ftp_path("ftp://harbor@10.0.0.2:2121/DCIM/a%20b.mp4") == "/DCIM/a b.mp4"
    assert fs.ftp_path("ftp://harbor@h/x%0d%0aDELE%20y") is None      # no command injection
    assert fs.ftp_path("sftp://h/a") is None
    assert fs.wants_choice("video/mp4", fs.STREAM_MIN)
    assert not fs.wants_choice("video/mp4", fs.STREAM_MIN - 1)
    assert not fs.wants_choice("application/pdf", fs.STREAM_MIN * 2)


def test_relay_serves_ranges(relay):
    url = relay.add("10.0.0.2", 2121, "harbor", "123456", "/DCIM/v.mp4", len(DATA), "v.mp4", "video/mp4")
    assert url.startswith("http://127.0.0.1:") and url.endswith("/v.mp4")
    assert relay.srv.server_address[0] == "127.0.0.1"
    st, h, body = get(url)
    assert st == 200 and body == DATA and h["Accept-Ranges"] == "bytes"
    st, h, body = get(url, "bytes=1000-1999")
    assert st == 206 and body == DATA[1000:2000]
    assert h["Content-Range"] == "bytes 1000-1999/%d" % len(DATA)
    assert relay.seen[-1] == ("/DCIM/v.mp4", 1000, "123456")
    st, h, body = get(url, method="HEAD")
    assert st == 200 and body == b"" and h["Content-Length"] == str(len(DATA))
    with pytest.raises(urllib.error.HTTPError) as e:
        get(url, "bytes=%d-" % len(DATA))
    assert e.value.code == 416


def test_relay_rejects_unknown_tokens(relay):
    url = relay.add("h", 21, "harbor", "1", "/a.mp3", 10, "a.mp3", "audio/mpeg")
    base = url.rsplit("/", 2)[0]
    for bad in (base + "/nope/a.mp3", base + "/", base + "/../etc/passwd"):
        with pytest.raises(urllib.error.HTTPError) as e:
            get(bad)
        assert e.value.code == 404
    for _ in range(fs.MAX_ITEMS):
        relay.add("h", 21, "harbor", "1", "/b.mp3", 10, "b.mp3", "audio/mpeg")
    with pytest.raises(urllib.error.HTTPError):
        get(url)                                       # oldest link dropped


def test_pins_stay_in_memory():
    fs.remember_pin("10.0.0.9", 2121, "654321")
    assert fs.pin_for("10.0.0.9", 2121) == "654321"
    fs.forget_pin("10.0.0.9", 2121)
    assert fs.pin_for("10.0.0.9", 2121) is None
