"""Harbor Files 0.5.0 helpers: new documents, duplicate names, folder
customization, network addresses, Office preview cache, share folders."""

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                                "src"))

import gi  # noqa: E402

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")

import pytest  # noqa: E402

from harbor import files_actions as fa  # noqa: E402
from harbor import files_customize as fcu  # noqa: E402
from harbor import files_network as fn  # noqa: E402
from harbor import files_office as fo  # noqa: E402
from harbor import files_share_ui as fsu  # noqa: E402


def test_new_document_never_overwrites(tmp_path):
    a = fa.new_document(str(tmp_path), "docx")
    b = fa.new_document(str(tmp_path), "docx")
    assert a != b and os.path.getsize(a) > 0
    with open(a, "rb") as f:
        assert f.read(2) == b"PK"                       # a real Office zip


def test_duplicate_name_and_exec_mode(tmp_path):
    p = str(tmp_path / "report.txt")
    assert fa.duplicate_name(p, set()).endswith("report - Copy.txt")
    assert fa.duplicate_name(p, {"report - Copy.txt"}).endswith("report - Copy (2).txt")
    assert fa.duplicate_name(str(tmp_path / ".bashrc"), set()).endswith(".bashrc - Copy")
    assert fa.make_executable_mode(0o644) == 0o755
    assert fa.make_executable_mode(0o600) == 0o700


def test_import_icon(tmp_path):
    src = tmp_path / "pic.png"
    src.write_bytes(b"\x89PNG fake")
    store = tmp_path / "store"
    a = fcu.import_icon(str(src), root=str(store))
    assert fcu.import_icon(str(src), root=str(store)) == a          # same content, same file
    assert oct(os.stat(a).st_mode & 0o777) == "0o644"
    bad = tmp_path / "x.exe"
    bad.write_bytes(b"MZ")
    with pytest.raises(ValueError):
        fcu.import_icon(str(bad), root=str(store))


def test_metadata_info_clears_bad_values():
    info = fcu.metadata_info(color="not a colour", emblem="nope")
    for a in info.list_attributes(None):
        assert not info.get_attribute_string(a)


def test_network_urls():
    assert fn.normalize_url("files.example.org") == "sftp://files.example.org/"
    assert fn.normalize_url("\\\\nas\\music") == "smb://nas/music"
    assert fn.normalize_url("ftp://me:secret@host/pub/") == "ftp://me@host/pub/"
    assert fn.normalize_url("webdav://host/x") == "dav://host/x"
    for bad in ("", "gopher://host/", "ftp:///nohost"):
        with pytest.raises(ValueError):
            fn.normalize_url(bad)
    assert fn.build_url("ftp", "host", 21, "pub", "me") == "ftp://me@host/pub/"
    assert fn.build_url("sftp", "fe80::1", 2222) == "sftp://[fe80::1]:2222/"
    assert fn.display_name("smb://nas/music/") == "music on nas"
    assert fn.display_name("sftp://host/", "Box") == "Box"


def test_office_cache_key(tmp_path):
    a = fo.cache_key("/x/a.docx", 1, 10)
    assert a != fo.cache_key("/x/a.docx", 2, 10) != fo.cache_key("/x/a.docx", 1, 10, "p2")
    (tmp_path / "k.png").write_bytes(b"12345")
    assert fo.cache_size(str(tmp_path)) == 5
    fo.clear_cache(str(tmp_path))
    assert fo.cache_size(str(tmp_path)) == 0


class _Settings:
    def __init__(self):
        self.data = {"share_folders": []}
        self.saved = 0

    def save(self):
        self.saved += 1


def test_share_folder_list():
    s = _Settings()
    assert fsu.add_share_folder(s, "/home/u/Music/")
    assert not fsu.add_share_folder(s, "/home/u/Music")              # no duplicates
    fsu.set_share_upload(s, "/home/u/Music", True)
    assert fsu.share_folders(s) == [{"path": "/home/u/Music", "upload": True}]
    fsu.remove_share_folder(s, "/home/u/Music")
    assert fsu.share_folders(s) == [] and s.saved == 3


def test_qr_texture_size():
    tex, side = fsu.qr_texture("http://192.168.1.20:8765/", scale=2)
    assert side == (25 + 8) * 2 and tex.get_width() == side
