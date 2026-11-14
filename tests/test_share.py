"""Share over Wi-Fi: the security rules in files_share's docstring."""

import base64
import http.client
import os
import random
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                                "src"))

import pytest  # noqa: E402

from harbor import files_share as fs  # noqa: E402
from harbor import qr  # noqa: E402


@pytest.fixture
def share(tmp_path):
    ro = tmp_path / "Music"
    rw = tmp_path / "Inbox"
    for d in (ro, rw):
        d.mkdir()
    (ro / "song.txt").write_text("la la")
    (ro / "page.html").write_text("<script>alert(1)</script>")
    (ro / ".secret").write_text("hidden")
    (ro / "sub").mkdir()
    (tmp_path / "outside.txt").write_text("nope")
    os.symlink(tmp_path / "outside.txt", ro / "link.txt")
    srv = fs.ShareServer([{"path": str(ro)}, {"path": str(rw), "upload": True}],
                         port=random.randint(20000, 40000), host="127.0.0.1", minutes=0)
    srv.start()
    yield srv, tmp_path
    srv.stop()
    assert not srv.running


def req(srv, method, path, body=None, headers=None):
    c = http.client.HTTPConnection(srv.host, srv.port, timeout=5)
    c.request(method, path, body=body, headers=headers or {})
    r = c.getresponse()
    data = r.read()
    c.close()
    return r, data


def login(srv, pin=None):
    r, _ = req(srv, "POST", "/login", "pin=%s" % (pin or srv.pin),
               {"Content-Type": "application/x-www-form-urlencoded"})
    return r


def cookie(srv):
    r = login(srv)
    assert r.status == 303
    sc = r.getheader("Set-Cookie")
    assert "HttpOnly" in sc and "SameSite=Strict" in sc
    return {"Cookie": sc.split(";")[0]}


def dav(srv, pin=None):
    tok = base64.b64encode(("harbor:%s" % (pin or srv.pin)).encode()).decode()
    return {"Authorization": "Basic " + tok}


def test_pin_and_token_shape(share):
    srv, _ = share
    assert len(srv.pin) == 6 and srv.pin.isdigit()
    assert len(srv.token) == 32


def test_needs_login(share):
    srv, _ = share
    r, _ = req(srv, "GET", "/f/Music/song.txt")
    assert r.status == 303 and r.getheader("Location") == "/login"
    r, _ = req(srv, "GET", "/", headers={"Cookie": "harbor_share=" + "0" * 32})
    assert r.status == 303


def test_security_headers(share):
    srv, _ = share
    r, _ = req(srv, "GET", "/login")
    assert "default-src 'none'" in r.getheader("Content-Security-Policy")
    assert r.getheader("X-Content-Type-Options") == "nosniff"
    assert r.getheader("Referrer-Policy") == "no-referrer"


def test_lockout_after_five_wrong(share):
    srv, _ = share
    wrong = "%06d" % ((int(srv.pin) + 1) % 1000000)
    for _ in range(5):
        assert login(srv, wrong).status == 403
    assert login(srv).status == 429          # even the right PIN is locked out
    r, _ = req(srv, "GET", "/dav/", headers=dav(srv))
    assert r.status == 429


def test_guard_unlocks_after_timeout():
    t = [0.0]
    g = fs.Guard("123456", now=lambda: t[0])
    for _ in range(5):
        assert g.check("000000") == "bad"
    assert g.check("123456") == "locked"
    t[0] += fs.LOCK_SECONDS + 1
    assert g.check("123456") == "ok"


def test_serves_and_confines(share):
    srv, _ = share
    h = cookie(srv)
    r, data = req(srv, "GET", "/f/Music/song.txt", headers=h)
    assert r.status == 200 and data == b"la la"
    for bad in ("/f/Music/.secret", "/f/Music/link.txt", "/f/Music/../outside.txt",
                "/f/Music/%2e%2e/outside.txt", "/f/Nope/song.txt", "/f/Music/sub/../../x"):
        r, _ = req(srv, "GET", bad, headers=h)
        assert r.status == 404, bad
    r, page = req(srv, "GET", "/f/Music/", headers=h)
    assert b"song.txt" in page and b".secret" not in page and b"link.txt" not in page


def test_html_is_a_download(share):
    srv, _ = share
    r, _ = req(srv, "GET", "/f/Music/page.html", headers=cookie(srv))
    assert r.getheader("Content-Disposition").startswith("attachment")
    assert r.getheader("Content-Type") == "application/octet-stream"


def test_range(share):
    srv, _ = share
    r, data = req(srv, "GET", "/f/Music/song.txt", headers={**cookie(srv), "Range": "bytes=1-2"})
    assert r.status == 206 and data == b"a "


def test_upload_rules(share):
    srv, tmp = share
    h = {**cookie(srv), "X-Harbor-Upload": "1"}
    r, _ = req(srv, "PUT", "/f/Music/new.txt", b"x", h)
    assert r.status == 403                                    # read-only folder
    r, _ = req(srv, "PUT", "/f/Inbox/a.txt", b"one", {"Cookie": h["Cookie"]})
    assert r.status == 403                                    # no upload header (CSRF)
    r, _ = req(srv, "PUT", "/f/Inbox/a.txt", b"one", {**h, "Origin": "http://evil.example"})
    assert r.status == 403
    for body in (b"one", b"two"):
        r, _ = req(srv, "PUT", "/f/Inbox/a.txt", body, h)
        assert r.status == 201
    assert (tmp / "Inbox" / "a.txt").read_bytes() == b"one"   # never overwritten
    assert (tmp / "Inbox" / "a (2).txt").read_bytes() == b"two"
    assert oct(os.stat(tmp / "Inbox" / "a (2).txt").st_mode & 0o777) == "0o644"
    for bad in ("/f/Inbox/.bashrc", "/f/Inbox/..%2fx"):
        r, _ = req(srv, "PUT", bad, b"x", h)
        assert r.status in (400, 404), bad
    assert not [n for n in os.listdir(tmp / "Inbox") if n.endswith(".part")]


def test_dav(share):
    srv, tmp = share
    r, _ = req(srv, "PROPFIND", "/dav/", headers={"Depth": "1"})
    assert r.status == 401
    r, _ = req(srv, "PROPFIND", "/dav/", headers={**dav(srv, "000000"), "Depth": "1"})
    assert r.status == 401
    r, body = req(srv, "PROPFIND", "/dav/Music/", headers={**dav(srv), "Depth": "1"})
    assert r.status == 207 and b"song.txt" in body and b".secret" not in body
    assert b"link.txt" not in body
    r, _ = req(srv, "PUT", "/dav/Music/x.txt", b"x", dav(srv))
    assert r.status == 403
    r, _ = req(srv, "PUT", "/dav/Inbox/d.txt", b"x", dav(srv))
    assert r.status == 201
    r, _ = req(srv, "PUT", "/dav/Inbox/d.txt", b"y", dav(srv))
    assert r.status == 412
    r, _ = req(srv, "MKCOL", "/dav/Inbox/New", headers=dav(srv))
    assert r.status == 201 and (tmp / "Inbox" / "New").is_dir()
    r, _ = req(srv, "DELETE", "/dav/Music/song.txt", headers=dav(srv))
    assert r.status == 403 and (tmp / "Music" / "song.txt").exists()


def test_helpers():
    assert fs.safe_name("a.txt") == "a.txt"
    for bad in ("", ".", "..", ".x", "a/b", "a\\b", "a\x00", "x" * 300):
        assert fs.safe_name(bad) is None
    assert set(fs.share_names(["/a/Music", "/b/Music"])) == {"Music", "Music (2)"}
    assert fs.disposition("/x/a.svg")[1] is False
    assert fs.disposition("/x/a.png")[1] is True


def test_lan_address_is_private():
    ip = fs.lan_address()
    if ip is not None:
        import ipaddress
        a = ipaddress.ip_address(ip)
        assert a.is_private and not a.is_loopback
    assert fs.lan_address("8.8.8.8") in (None, ip)


def test_qr_shape():
    m = qr.matrix("http://192.168.1.20:8765/")
    assert len(m) == 25 and all(len(r) == 25 for r in m)       # version 2
    assert m[0][:7] == [True] * 7                               # finder pattern
    with pytest.raises(ValueError):
        qr.matrix("x" * 400)


# --- QR codes (0.5.6) -----------------------------------------------------

def test_qr_text_carries_pin(tmp_path):
    from harbor.files_share import ShareServer
    s = ShareServer([{"path": str(tmp_path)}], host="192.168.1.5", minutes=0)
    assert s.qr_text == "http://192.168.1.5:8765/#pin=" + s.pin
    st = s.status()
    assert st["pin"] == s.pin and st["dav_url"] == s.dav_url


def test_parse_phone_qr():
    from harbor.files_scan import parse_phone_qr
    assert parse_phone_qr("ftp://harbor:123456@10.0.0.7:2121/") == (
        "ftp://harbor@10.0.0.7:2121/", "harbor", "123456")
    assert parse_phone_qr("http://10.0.0.7:8765/#pin=1") is None
    assert parse_phone_qr("hello") is None
    assert parse_phone_qr("ftp://h:1@x:99999/") is None


def test_share_single_file(tmp_path):
    (tmp_path / "app.apk").write_bytes(b"PK apk")
    (tmp_path / "other.txt").write_text("not shared")
    srv = fs.ShareServer([{"path": str(tmp_path / "app.apk"), "upload": True}],
                         port=random.randint(20000, 40000), host="127.0.0.1", minutes=0)
    srv.start()
    try:
        assert srv.upload == {"app.apk": False}
        c = cookie(srv)
        r, body = req(srv, "GET", "/", headers=c)
        assert r.status == 200 and b"app.apk" in body and b"/f/app.apk?dl=1" in body
        r, body = req(srv, "GET", "/f/app.apk", headers=c)
        assert r.status == 200 and body == b"PK apk"
        for bad in ("/f/app.apk/x", "/f/app.apk/../other.txt", "/f/other.txt"):
            r, body = req(srv, "GET", bad, headers=c)
            assert r.status == 404 and b"not shared" not in body
        r, body = req(srv, "PROPFIND", "/dav/", headers={**dav(srv), "Depth": "1"})
        assert r.status == 207 and b"<D:href>/dav/app.apk</D:href>" in body
        r, body = req(srv, "GET", "/dav/app.apk", headers=dav(srv))
        assert r.status == 200 and body == b"PK apk"
        r, _ = req(srv, "PUT", "/dav/app.apk/x.txt", b"x", dav(srv))
        assert r.status in (400, 403)
        r, _ = req(srv, "DELETE", "/dav/app.apk", headers=dav(srv))
        assert r.status == 404 and (tmp_path / "app.apk").exists()
    finally:
        srv.stop()


def test_right_click_share_is_one_off(tmp_path, monkeypatch):
    """Sharing a selected item shares only it and leaves the saved list alone."""
    import types
    import gi
    gi.require_version("Gtk", "4.0")
    gi.require_version("Adw", "1")
    from gi.repository import Adw
    from harbor import common, files_core as fc
    from harbor.files_share_ui import ShareDialog, add_share_folder, share_folders
    Adw.init()
    monkeypatch.setattr(common, "DRY_RUN", True)
    saved, one = tmp_path / "saved", tmp_path / "one"
    saved.mkdir()
    one.mkdir()
    s = fc.Settings(str(tmp_path / "files.json"))
    add_share_folder(s, str(saved))
    win = types.SimpleNamespace(app=types.SimpleNamespace(settings=s),
                                notify_toast=lambda *a, **k: None)
    common.DRY_RUN_LOG.clear()
    ShareDialog(win, add_paths=[str(one)])
    assert common.DRY_RUN_LOG[-1] == ["share-start", [str(one)]]
    assert [f["path"] for f in share_folders(s)] == [str(saved)]
    common.DRY_RUN_LOG.clear()
    ShareDialog(win).start()
    assert common.DRY_RUN_LOG[-1] == ["share-start", [str(saved)]]
