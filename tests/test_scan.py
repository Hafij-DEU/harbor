"""Scan dialog helpers: camera picking and phone pairing."""

import http.client
import json
import os
import sys
import threading

sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                                "src"))

from harbor import files_scan as sc  # noqa: E402


def _node(root, name, index, virtual, label):
    d = root / "class" / name
    d.mkdir(parents=True)
    dev = root / "devices" / ("virtual/video4linux" if virtual else "pci0/usb1") / name
    dev.mkdir(parents=True)
    (d / "device").symlink_to(dev)
    (d / "index").write_text("%d\n" % index)
    (d / "name").write_text(label + "\n")


def test_list_cameras_skips_loopback_and_metadata(tmp_path):
    _node(tmp_path, "video0", 0, True, "MeshDrop camera")
    _node(tmp_path, "video1", 0, False, "Integrated Camera")
    _node(tmp_path, "video2", 1, False, "Integrated Camera")
    _node(tmp_path, "video10", 0, False, "USB Cam")
    got = sc.list_cameras(str(tmp_path / "class"))
    assert got == [("/dev/video1", "Integrated Camera"), ("/dev/video10", "USB Cam")]


FTP = "ftp://harbor:1234@127.0.0.1:2121/"


def _body(token, ftp=FTP):
    return json.dumps({"token": token, "ftp": ftp}).encode()


def test_check_pair():
    assert sc.check_pair("tok", "/pair", _body("tok"), "127.0.0.1") == (200, FTP)
    assert sc.check_pair("tok", "/pair", _body("bad"), "127.0.0.1")[0] == 403
    assert sc.check_pair("tok", "/pair", _body("tok"), "10.0.0.9")[0] == 403
    assert sc.check_pair("tok", "/pair", _body("tok", "http://x/"), "127.0.0.1")[0] == 400
    assert sc.check_pair("tok", "/pair", b"{", "127.0.0.1")[0] == 400
    assert sc.check_pair("tok", "/other", _body("tok"), "127.0.0.1")[0] == 404


def _post(port, body):
    c = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
    c.request("POST", "/pair", body, {"Content-Type": "application/json"})
    r = c.getresponse()
    r.read()
    c.close()
    return r.status


def test_pair_listener_one_shot():
    got, ev = [], threading.Event()
    pl = sc.PairListener(lambda f: (got.append(f), ev.set()), host="127.0.0.1")
    port = pl.srv.server_address[1]
    assert pl.url.startswith("harborpair://127.0.0.1:%d/" % port)
    assert pl.url.endswith("/" + pl.token)
    assert _post(port, _body("wrong")) == 403
    assert _post(port, _body(pl.token)) == 200
    assert ev.wait(5) and got == [FTP]
    pl.thread.join(5)
    assert not pl.thread.is_alive()


def test_pair_listener_closes_after_bad_tokens():
    pl = sc.PairListener(lambda f: None, host="127.0.0.1")
    port = pl.srv.server_address[1]
    for _ in range(sc.PAIR_MAX_BAD):
        assert _post(port, _body("nope")) == 403
    pl.thread.join(5)
    assert not pl.thread.is_alive()
