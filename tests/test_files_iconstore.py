"""Customize folder: Harbor-only store, undo history, safe pictures, global export."""
import os
import stat

import gi
import pytest

gi.require_version("Gtk", "4.0")
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf, Gio, GLib  # noqa: E402

from harbor import files_iconstore as ics  # noqa: E402
from harbor import files_icons as fi  # noqa: E402


def png(path, size=32):
    GdkPixbuf.Pixbuf.new(GdkPixbuf.Colorspace.RGB, True, 8, size, size).savev(str(path), "png", [], [])
    return str(path)


@pytest.fixture
def store(tmp_path):
    s = ics.reset_store_for_tests(str(tmp_path / "cfg" / "folder-icons.json"))
    yield s
    ics.reset_store_for_tests(None)


def test_clean_state_drops_bad_values(tmp_path):
    pic = png(tmp_path / "a.png")
    st = ics.clean_state({"icon_uri": GLib.filename_to_uri(pic, None), "icon_name": "folder-music",
                          "color": "#ABCDEF", "emblem": "nope"})
    assert st == {"icon_uri": GLib.filename_to_uri(pic, None), "color": "#abcdef"}   # picture wins
    assert ics.clean_state({"icon_name": "../../etc/passwd", "color": "red"}) is None
    assert ics.clean_state({"icon_name": "folder music"}) is None
    assert ics.clean_state("junk") is None


def test_safe_icon_path_refuses_unsafe_refs(tmp_path):
    pic = png(tmp_path / "ok.png")
    assert ics.safe_icon_path(GLib.filename_to_uri(pic, None)) == pic
    assert ics.safe_icon_path(pic) == pic
    big = tmp_path / "big.png"
    with open(big, "wb") as f:
        f.truncate(ics.PICTURE_MAX + 1)
    fifo = tmp_path / "pipe.png"
    os.mkfifo(fifo)
    (tmp_path / "dir.png").mkdir()
    (tmp_path / "x.exe").write_bytes(b"MZ")
    for bad in ("http://example.org/a.png", "sftp://host/a.png", "a.png", "", None,
                str(big), str(fifo), str(tmp_path / "dir.png"), str(tmp_path / "x.exe"),
                str(tmp_path / "missing.png"), "/tmp/a\x00.png"):
        assert ics.safe_icon_path(bad) is None, bad


def test_store_roundtrip_private_and_tolerant(tmp_path, store):
    folder = str(tmp_path / "Music")
    store.put(folder, {"icon_name": "folder-music", "color": "#112233"})
    store.save()
    assert stat.S_IMODE(os.stat(store.path).st_mode) == 0o600
    again = ics.Store(store.path)
    assert again.get(folder + "/") == {"icon_name": "folder-music", "color": "#112233"}
    store.put(folder, None)
    assert store.get(folder) is None
    with pytest.raises(ValueError):
        store.put("relative/path", {"color": "#112233"})
    with open(store.path, "w") as f:
        f.write("{not json")
    assert ics.Store(store.path).items == {}            # corrupt file: start empty, no crash


def test_history_undo_order_and_limit(tmp_path, store):
    folder = str(tmp_path / "Docs")
    assert not store.can_undo(folder)
    for i in range(ics.HISTORY_MAX + 3):
        store.push(folder, "harbor", {"color": "#%06x" % i})
    store.push(folder, "global", None)
    assert len(store.history[ics.key_for(folder)]) == ics.HISTORY_MAX
    assert store.pop(folder) == {"scope": "global", "state": None}
    assert store.pop(folder)["state"] == {"color": "#%06x" % (ics.HISTORY_MAX + 2)}
    store.push(folder, "bogus", {"color": "#000000"})        # unknown place ignored
    assert store.pop(folder)["scope"] == "harbor"


def test_import_picture_reencodes(tmp_path):
    root = str(tmp_path / "icons")
    a = ics.import_picture(png(tmp_path / "big.png", 600), root=root)
    assert a.endswith(".png") and stat.S_IMODE(os.stat(a).st_mode) == 0o644
    pix = GdkPixbuf.Pixbuf.new_from_file(a)
    assert max(pix.get_width(), pix.get_height()) == ics.ICON_PX       # scaled down
    assert ics.import_picture(png(tmp_path / "big2.png", 600), root=root) == a   # same content
    # an SVG with a script and an external reference becomes a plain PNG
    svg = tmp_path / "evil.svg"
    svg.write_text('<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" '
                   'width="64" height="64"><script>alert(1)</script>'
                   '<image xlink:href="file:///etc/passwd" width="10" height="10"/>'
                   '<rect width="64" height="64" fill="red"/></svg>')
    out = ics.import_picture(str(svg), root=root)
    assert out.endswith(".png") and b"<script" not in open(out, "rb").read()


def test_import_picture_rejects(tmp_path):
    root = str(tmp_path / "icons")
    fake = tmp_path / "fake.png"
    fake.write_bytes(b"\x89PNG not really")
    big = tmp_path / "big.jpg"
    with open(big, "wb") as f:
        f.truncate(ics.PICTURE_MAX + 1)
    fifo = tmp_path / "pipe.png"
    os.mkfifo(fifo)
    (tmp_path / "x.exe").write_bytes(b"MZ")
    for bad in (str(fake), str(big), str(fifo), str(tmp_path / "x.exe"), "rel.png",
                str(tmp_path / "missing.png")):
        with pytest.raises(ValueError):
            ics.import_picture(bad, root=root)
    assert not os.path.exists(root) or not os.listdir(root)


def test_global_values_export_and_names(tmp_path, monkeypatch):
    monkeypatch.setattr(ics, "icons_dir", lambda: str(tmp_path / "icons"))
    assert ics.global_values({"icon_name": "folder-music"}) == (None, "folder-music")
    uri, name = ics.global_values({"color": "#ff0000"})
    assert name is None and uri.startswith("file://") and uri.endswith(".svg")
    path, _h = GLib.filename_from_uri(uri)
    assert "#ff0000" in open(path).read().lower() or os.path.getsize(path) > 0
    pic = png(tmp_path / "p.png")
    assert ics.global_values({"icon_uri": GLib.filename_to_uri(pic, None)}) == (
        GLib.filename_to_uri(pic, None), None)
    assert ics.global_values(None) == (None, None)


def test_harbor_only_icon_wins_inside_harbor(tmp_path, store):
    folder = str(tmp_path / "Pics")
    info = Gio.FileInfo()
    info.set_attribute_string(fi.ATTR_CUSTOM_ICON_NAME, "folder-music")
    assert fi.custom_of(info, folder)[1] == "folder-music"            # global
    store.put(folder, {"icon_name": "folder-pictures"})
    assert fi.custom_of(info, folder)[1] == "folder-pictures"         # Harbor-only wins
    assert fi.custom_of(info, None)[1] == "folder-music"              # no path: global


def test_unsafe_global_metadata_is_ignored(tmp_path):
    info = Gio.FileInfo()
    info.set_attribute_string(fi.ATTR_CUSTOM_ICON, "http://example.org/x.png")
    info.set_attribute_string(fi.ATTR_FOLDER_COLOR, "url(evil)")
    assert fi.global_of(info) == (None, None, None, None)
    assert fi.custom_gicon(info, True) is None
