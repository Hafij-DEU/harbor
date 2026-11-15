"""0.5.16: batch rename, folder sizes, phone names, space usage."""
import os

from harbor import files_core as fc
from harbor import files_rename as fr
from harbor import files_space
from harbor.files_sidebar import PHONE_NAMES, parse_banner, phone_label


def test_plan_find_replace_and_ext_kept():
    out = fr.plan(["IMG_001.JPG", "IMG_002.JPG"], find="img_", replace="Trip ")
    assert out == [("Trip 001.JPG", None), ("Trip 002.JPG", None)]
    out = fr.plan(["IMG_001.JPG"], find="img_", replace="x", match_case=True)
    assert out == [("IMG_001.JPG", None)]


def test_plan_pattern_numbering_and_case():
    out = fr.plan(["a.txt", "b.txt", "c.txt"], pattern="Note ###", start=9)
    assert [n for n, _ in out] == ["Note 009.txt", "Note 010.txt", "Note 011.txt"]
    out = fr.plan(["My File.TXT"], pattern="* - ##", case="lowercase")
    assert out == [("my file - 01.TXT", None)]
    out = fr.plan(["hello world.md"], case="Title Case")
    assert out == [("Hello World.md", None)]


def test_plan_regex_and_errors():
    out = fr.plan(["2024-05-01 x.jpg"], find=r"(\d+)-(\d+)-(\d+)", replace=r"\3.\2.\1", regex=True)
    assert out == [("01.05.2024 x.jpg", None)]
    assert fr.plan(["a"], find="(", regex=True)[0][1].startswith("Bad pattern")
    out = fr.plan(["a.txt", "b.txt"], pattern="same")
    assert all(e == "Two items would get this name" for _, e in out)
    out = fr.plan(["a.txt"], find="a", replace="b", taken={"b.txt"})
    assert out[0][1] == "An item with this name is already here"
    assert fr.plan(["a.txt"], find="a.txt", replace="", include_ext=True)[0][1]


def test_apply_swaps_and_undo(tmp_path):
    for n in ("a", "b"):
        (tmp_path / n).write_text(n)
    done, errors = fr.apply(str(tmp_path), [("a", "b"), ("b", "a")])
    assert not errors and len(done) == 2
    assert (tmp_path / "a").read_text() == "b" and (tmp_path / "b").read_text() == "a"
    assert not [p for p in os.listdir(tmp_path) if p.startswith(".hb-rename-")]


def test_dir_size(tmp_path):
    (tmp_path / "d" / "e").mkdir(parents=True)
    (tmp_path / "d" / "x").write_bytes(b"1" * 100)
    (tmp_path / "d" / "e" / "y").write_bytes(b"2" * 50)
    os.symlink("/usr", tmp_path / "d" / "link")
    # the link counts as itself (4 bytes), /usr is not followed
    assert fc.dir_size(str(tmp_path / "d")) == (154, True)
    assert fc.dir_size(str(tmp_path / "nope")) == (None, False)
    assert fc.dir_size(str(tmp_path / "d"), cancelled=lambda: True) == (None, False)
    size, complete = fc.dir_size(str(tmp_path / "d"), max_entries=1)
    assert not complete


def test_space_scan(tmp_path):
    (tmp_path / "big").mkdir()
    (tmp_path / "big" / "f").write_bytes(b"x" * 500)
    (tmp_path / "small.txt").write_bytes(b"x" * 10)
    rows = files_space.scan(str(tmp_path))
    assert [(r[0], r[2], r[3]) for r in rows] == [("big", True, 500), ("small.txt", False, 10)]


def test_phone_banner_and_label():
    assert parse_banner("220 Harbor Files FTP ready (Redmi Pad 2)\r\n") == "Redmi Pad 2"
    assert parse_banner("220 Harbor Files FTP ready") is None
    assert parse_banner("220 vsftpd 3.0") is None
    assert parse_banner(None) is None
    uri = "ftp://harbor@192.168.1.101:2121/"
    PHONE_NAMES.pop(uri, None)
    assert phone_label(uri) == "Phone (192.168.1.101)"
    PHONE_NAMES[uri] = "Redmi Pad 2"
    assert phone_label(uri) == "Redmi Pad 2"
    PHONE_NAMES.pop(uri)
    assert phone_label("smb://host/share") is None


def test_drive_names_saved_and_cleaned(tmp_path):
    """0.5.35: Harbor-only drive names."""
    p = str(tmp_path / "files.json")
    s = fc.Settings(p)
    s.set_drive_name("uuid:ABC", "  Games  ")
    s.set_drive_name("root", "System")
    assert fc.Settings(p).drive_name("uuid:ABC") == "Games"
    s.set_drive_name("root", "")
    assert fc.Settings(p).drive_name("root") is None
    with open(p, "w") as f:
        f.write('{"drive_names": {"a": 3, "b": " ", "c": "OK"}}')
    assert fc.Settings(p).get("drive_names") == {"c": "OK"}


def test_drive_id_and_rename_dialog_imports():
    from harbor.files_drive_rename import RenameDriveDialog, _msg  # noqa: F401
    from harbor.files_sidebar import drive_id
    assert drive_id(None) == "root"

    class E:
        message = "GDBus.Error:org.freedesktop.UDisks2.Error.Failed: Busy"
    assert _msg(E()) == "Busy"
