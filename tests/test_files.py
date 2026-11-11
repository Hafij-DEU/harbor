"""Tests for Harbor Files logic. Only temporary folders are touched."""

import os
import subprocess
import sys
import textwrap
import threading

os.environ["HARBOR_DRY_RUN"] = "1"
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "src"))

import gi  # noqa: E402

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
import pytest  # noqa: E402

from harbor import files_core as fc  # noqa: E402


def touch(path, data=b""):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(data)


def read(path):
    with open(path, "rb") as f:
        return f.read()


# ------------------------------------------------------------------ names

def test_split_ext():
    assert fc.split_ext("photo.jpg") == ("photo", ".jpg")
    assert fc.split_ext(".bashrc") == (".bashrc", "")
    assert fc.split_ext("Makefile") == ("Makefile", "")
    assert fc.split_ext("a.tar.gz") == ("a", ".tar.gz")
    assert fc.split_ext("trailing.") == ("trailing.", "")


def test_unique_name_basic():
    taken = {"a.txt", "a (2).txt"}
    assert fc.unique_name("b.txt", taken.__contains__) == "b.txt"
    assert fc.unique_name("a.txt", taken.__contains__) == "a (3).txt"


def test_unique_name_counts_on_and_dirs():
    taken = {"x (4).txt"}
    assert fc.unique_name("x (4).txt", taken.__contains__) == "x (5).txt"
    taken = {"New folder", "New folder (2)"}
    assert fc.unique_name("New folder", taken.__contains__, is_dir=True) == "New folder (3)"
    taken = {"v1.2"}
    assert fc.unique_name("v1.2", taken.__contains__, is_dir=True) == "v1.2 (2)"
    assert fc.unique_name("v1.2", taken.__contains__) == "v1 (2).2"


def test_validate_name():
    exists = {"taken.txt"}.__contains__
    assert fc.validate_name("ok.txt", exists) is None
    assert fc.validate_name("", exists)
    assert fc.validate_name("   ", exists)
    assert fc.validate_name("a/b", exists)
    assert fc.validate_name("..", exists)
    assert fc.validate_name("x" * 300, exists)
    assert fc.validate_name("taken.txt", exists)
    assert fc.validate_name("taken.txt", exists, old="taken.txt") is None


def test_rename_selection_range():
    assert fc.rename_selection_range("report.final.pdf") == (0, len("report.final"))
    assert fc.rename_selection_range("folder.d", is_dir=True) == (0, 8)


# ------------------------------------------------------------------ sorting

def E(name, is_dir=False, mtime=0, type="", size=0):
    return dict(name=name, is_dir=is_dir, mtime=mtime, type=type, size=size)


def test_natural_sort_and_folders_first():
    items = [E("file10"), E("File2"), E("zdir", True), E("adir", True), E("file1")]
    out = [e["name"] for e in fc.sort_entries(items)]
    assert out == ["adir", "zdir", "file1", "File2", "file10"]


def test_sort_descending_keeps_folders_first():
    items = [E("b"), E("a"), E("d1", True), E("d2", True)]
    out = [e["name"] for e in fc.sort_entries(items, "name", descending=True)]
    assert out == ["d2", "d1", "b", "a"]


def test_sort_by_size_date_type():
    items = [E("big", size=900, mtime=1, type="Text"), E("small", size=5, mtime=3, type="Image"),
             E("mid", size=50, mtime=2, type="Audio")]
    assert [e["name"] for e in fc.sort_entries(items, "size")] == ["small", "mid", "big"]
    assert [e["name"] for e in fc.sort_entries(items, "modified")] == ["big", "mid", "small"]
    assert [e["name"] for e in fc.sort_entries(items, "type")] == ["mid", "small", "big"]


# ------------------------------------------------------------------ paths

def test_breadcrumbs_under_home():
    c = fc.breadcrumbs("/home/u/Documents/Work", home="/home/u")
    assert c == [("Home", "/home/u"), ("Documents", "/home/u/Documents"),
                 ("Work", "/home/u/Documents/Work")]
    assert fc.breadcrumbs("/home/u", home="/home/u") == [("Home", "/home/u")]


def test_breadcrumbs_outside_home():
    c = fc.breadcrumbs("/usr/share", home="/home/u")
    assert c == [("This computer", "/"), ("usr", "/usr"), ("share", "/usr/share")]
    assert fc.breadcrumbs("/", home="/home/u") == [("This computer", "/")]
    # a sibling with the home name as prefix is not "under home"
    assert fc.breadcrumbs("/home/user2", home="/home/u")[0] == ("This computer", "/")


def test_parse_location():
    h = "/home/u"
    assert fc.parse_location("~", home=h) == h
    assert fc.parse_location("~/Music/", home=h) == "/home/u/Music"
    assert fc.parse_location("sub", cwd="/tmp", home=h) == "/tmp/sub"
    assert fc.parse_location("../x", cwd="/tmp/a", home=h) == "/tmp/x"
    assert fc.parse_location("file:///usr/share/my%20dir", home=h) == "/usr/share/my dir"
    assert fc.parse_location("trash:///", home=h) == "trash"
    assert fc.parse_location("Home", home=h) == "home"
    assert fc.parse_location("sftp://host/path", home=h) == "sftp://host/path"
    assert fc.parse_location("  ", home=h) is None


def test_is_inside():
    assert fc.is_inside("/a/b/c", "/a/b")
    assert fc.is_inside("/a/b", "/a/b")
    assert not fc.is_inside("/a/bc", "/a/b")


def test_display_name():
    assert fc.display_name_for("/home/u", home="/home/u") == "Home"
    assert fc.display_name_for("/", home="/home/u") == "This computer"
    assert fc.display_name_for("/usr/share/", home="/home/u") == "share"


# ------------------------------------------------------------------ settings / pins

def test_pins_persist(tmp_path):
    p = str(tmp_path / "cfg" / "files.json")
    s = fc.Settings(p)
    assert s.pins == []
    assert s.pin("/data/one/")
    assert not s.pin("/data/one")  # no duplicates, normalised
    s.pin("/data/two")
    s.set("view", "icons")
    s2 = fc.Settings(p)
    assert s2.pins == ["/data/one", "/data/two"]
    assert s2.is_pinned("/data/two")
    assert s2.get("view") == "icons"
    assert s2.unpin("/data/one")
    assert fc.Settings(p).pins == ["/data/two"]


def test_settings_ignores_garbage(tmp_path):
    p = tmp_path / "files.json"
    p.write_text('{"pins": [1, "/ok"], "view": 5, "unknown": true}')
    s = fc.Settings(str(p))
    assert s.pins == ["/ok"]
    assert s.get("view") == "details"
    p.write_text("not json")
    assert fc.Settings(str(p)).pins == []


# ------------------------------------------------------------------ search

def test_matcher():
    m = fc.make_matcher("Rep")
    assert m("Report.pdf") and m("my_report") and not m("other")
    m = fc.make_matcher("*.PDF")
    assert m("a.pdf") and not m("a.pdf.txt")
    m = fc.make_matcher("tax 2024")
    assert m("2024 Tax return.ods") and not m("tax 2023")
    assert not fc.make_matcher("")("anything")


def test_search_walk(tmp_path):
    touch(str(tmp_path / "a" / "notes.txt"))
    touch(str(tmp_path / "a" / "b" / "Notes-old.md"))
    touch(str(tmp_path / ".hidden" / "notes.txt"))
    touch(str(tmp_path / "other.txt"))
    res = sorted(os.path.relpath(p, tmp_path) for p in fc.search_walk(str(tmp_path), "notes"))
    assert res == ["a/b/Notes-old.md", "a/notes.txt"]
    res = list(fc.search_walk(str(tmp_path), "notes", show_hidden=True))
    assert len(res) == 3
    ev = threading.Event()
    ev.set()
    assert list(fc.search_walk(str(tmp_path), "notes", cancel=ev)) == []


# ------------------------------------------------------------------ clipboard

def test_clipboard_formats_roundtrip():
    paths = ["/tmp/a b.txt", "/tmp/c"]
    payload = fc.gnome_copied_files(paths, cut=True)
    assert payload.startswith("cut\nfile:///tmp/a%20b.txt")
    assert fc.parse_copied_files(payload) == (True, paths)
    assert fc.parse_copied_files(fc.gnome_copied_files(paths, False)) == (False, paths)
    ul = fc.uri_list(paths)
    assert ul.endswith("\r\n")
    assert fc.parse_copied_files("# comment\r\n" + ul) == (False, paths)
    assert fc.parse_copied_files("http://x/y\n") == (False, [])
    assert fc.parse_copied_files("") == (False, [])


def test_quote_path():
    assert fc.quote_path("/a b/c") == '"/a b/c"'
    assert fc.quote_path("/a/c") == "/a/c"


# ------------------------------------------------------------------ copy / move

def job(choice=fc.KEEP_BOTH, all_=False, log=None):
    def ask(src, dst):
        if log is not None:
            log.append((os.path.basename(src), os.path.basename(dst)))
        return choice, all_
    return fc.Job(ask=ask)


def test_copy_file_and_folder(tmp_path):
    src, dst = tmp_path / "src", tmp_path / "dst"
    touch(str(src / "f.txt"), b"hello")
    touch(str(src / "d" / "inner" / "x.bin"), b"12345")
    os.mkdir(dst)
    j = fc.transfer(job(), [str(src / "f.txt"), str(src / "d")], str(dst))
    assert not j.errors
    assert read(str(dst / "f.txt")) == b"hello"
    assert read(str(dst / "d" / "inner" / "x.bin")) == b"12345"
    assert (src / "f.txt").exists()  # copy keeps the source
    assert j.done == j.total == 10


def test_conflict_keep_both_is_default(tmp_path):
    touch(str(tmp_path / "s" / "f.txt"), b"new")
    touch(str(tmp_path / "d" / "f.txt"), b"old")
    j = fc.transfer(fc.Job(), [str(tmp_path / "s" / "f.txt")], str(tmp_path / "d"))
    assert read(str(tmp_path / "d" / "f.txt")) == b"old"
    assert read(str(tmp_path / "d" / "f (2).txt")) == b"new"
    assert j.copied == [str(tmp_path / "d" / "f (2).txt")]


def test_conflict_skip_and_replace(tmp_path):
    touch(str(tmp_path / "s" / "f.txt"), b"new")
    touch(str(tmp_path / "d" / "f.txt"), b"old")
    log = []
    j = fc.transfer(job(fc.SKIP, log=log), [str(tmp_path / "s" / "f.txt")], str(tmp_path / "d"))
    assert log == [("f.txt", "f.txt")]
    assert j.skipped == 1
    assert read(str(tmp_path / "d" / "f.txt")) == b"old"
    fc.transfer(job(fc.REPLACE), [str(tmp_path / "s" / "f.txt")], str(tmp_path / "d"))
    assert read(str(tmp_path / "d" / "f.txt")) == b"new"
    assert sorted(os.listdir(tmp_path / "d")) == ["f.txt"]


def test_replace_folder_merges(tmp_path):
    touch(str(tmp_path / "s" / "dir" / "a.txt"), b"A2")
    touch(str(tmp_path / "s" / "dir" / "b.txt"), b"B")
    touch(str(tmp_path / "d" / "dir" / "a.txt"), b"A1")
    touch(str(tmp_path / "d" / "dir" / "keep.txt"), b"K")
    log = []
    fc.transfer(job(fc.REPLACE, all_=True, log=log), [str(tmp_path / "s" / "dir")],
                str(tmp_path / "d"))
    assert log == [("dir", "dir")]  # asked once, applied to all
    d = tmp_path / "d" / "dir"
    assert sorted(os.listdir(d)) == ["a.txt", "b.txt", "keep.txt"]
    assert read(str(d / "a.txt")) == b"A2"


def test_cancel_from_conflict_dialog(tmp_path):
    touch(str(tmp_path / "s" / "f.txt"), b"new")
    touch(str(tmp_path / "s" / "g.txt"), b"g")
    touch(str(tmp_path / "d" / "f.txt"), b"old")
    j = fc.transfer(job(None), [str(tmp_path / "s" / "f.txt"), str(tmp_path / "s" / "g.txt")],
                    str(tmp_path / "d"))
    assert j.cancelled
    assert sorted(os.listdir(tmp_path / "d")) == ["f.txt"]


def test_copy_into_same_folder_makes_copy(tmp_path):
    touch(str(tmp_path / "f.txt"), b"x")
    os.mkdir(tmp_path / "sub")
    j = fc.transfer(fc.Job(ask=lambda s, d: pytest.fail("should not ask")),
                    [str(tmp_path / "f.txt"), str(tmp_path / "sub")], str(tmp_path))
    assert sorted(os.listdir(tmp_path)) == ["f (2).txt", "f.txt", "sub", "sub (2)"]
    assert not j.errors


def test_move_same_fs(tmp_path):
    touch(str(tmp_path / "s" / "f.txt"), b"x")
    touch(str(tmp_path / "s" / "dir" / "y.txt"), b"y")
    os.mkdir(tmp_path / "d")
    j = fc.transfer(fc.Job(), [str(tmp_path / "s" / "f.txt"), str(tmp_path / "s" / "dir")],
                    str(tmp_path / "d"), move=True)
    assert not j.errors
    assert os.listdir(tmp_path / "s") == []
    assert read(str(tmp_path / "d" / "dir" / "y.txt")) == b"y"


def test_move_into_same_folder_is_noop(tmp_path):
    touch(str(tmp_path / "f.txt"), b"x")
    j = fc.transfer(fc.Job(), [str(tmp_path / "f.txt")], str(tmp_path), move=True)
    assert os.listdir(tmp_path) == ["f.txt"] and j.skipped == 1


def test_move_merge_with_skip_keeps_source_leftovers(tmp_path):
    touch(str(tmp_path / "s" / "dir" / "a.txt"), b"new")
    touch(str(tmp_path / "s" / "dir" / "b.txt"), b"b")
    touch(str(tmp_path / "d" / "dir" / "a.txt"), b"old")
    answers = iter([(fc.REPLACE, False), (fc.SKIP, False)])
    j = fc.transfer(fc.Job(ask=lambda s, d: next(answers)), [str(tmp_path / "s" / "dir")],
                    str(tmp_path / "d"), move=True)
    assert not j.errors
    assert read(str(tmp_path / "d" / "dir" / "a.txt")) == b"old"
    assert read(str(tmp_path / "d" / "dir" / "b.txt")) == b"b"
    assert os.listdir(tmp_path / "s" / "dir") == ["a.txt"]  # skipped item stays


def test_folder_into_itself_refused(tmp_path):
    touch(str(tmp_path / "dir" / "sub" / "x"), b"")
    j = fc.transfer(fc.Job(), [str(tmp_path / "dir")], str(tmp_path / "dir" / "sub"))
    assert j.errors and "itself" in j.errors[0]
    assert os.listdir(tmp_path / "dir" / "sub") == ["x"]


def test_symlink_copied_as_link(tmp_path):
    touch(str(tmp_path / "s" / "real.txt"), b"r")
    os.symlink("real.txt", tmp_path / "s" / "link")
    os.mkdir(tmp_path / "d")
    fc.transfer(fc.Job(), [str(tmp_path / "s" / "link")], str(tmp_path / "d"))
    assert os.path.islink(tmp_path / "d" / "link")


def test_cancel_before_start(tmp_path):
    touch(str(tmp_path / "s" / "f.txt"), b"x")
    os.mkdir(tmp_path / "d")
    j = fc.Job()
    j.cancel()
    fc.transfer(j, [str(tmp_path / "s" / "f.txt")], str(tmp_path / "d"))
    assert os.listdir(tmp_path / "d") == []


def test_progress_reported(tmp_path):
    touch(str(tmp_path / "s" / "f.bin"), b"x" * 5000)
    os.mkdir(tmp_path / "d")
    seen = []
    fc.transfer(fc.Job(progress=lambda d, t, n: seen.append((d, t))),
                [str(tmp_path / "s" / "f.bin")], str(tmp_path / "d"))
    assert seen and seen[-1] == (5000, 5000)


# ------------------------------------------------------------------ delete / trash / new / rename

def test_delete_permanently(tmp_path):
    touch(str(tmp_path / "dir" / "a" / "b.txt"))
    touch(str(tmp_path / "f.txt"))
    j = fc.delete_permanently(fc.Job(), [str(tmp_path / "dir"), str(tmp_path / "f.txt")])
    assert not j.errors and os.listdir(tmp_path) == []


def test_trash_paths_uses_given_trash(tmp_path):
    touch(str(tmp_path / "f.txt"))
    calls = []
    j = fc.trash_paths(fc.Job(), [str(tmp_path / "f.txt")],
                       trash=lambda g, c: calls.append(g.get_path()))
    assert calls == [str(tmp_path / "f.txt")] and j.copied == calls


def test_real_trash_goes_to_private_data_dir(tmp_path):
    """Gio trash with XDG_DATA_HOME pointing into tmp_path (never the real trash)."""
    data = tmp_path / "data"
    work = tmp_path / "work"
    touch(str(work / "gone.txt"), b"bye")
    code = textwrap.dedent("""
        import sys
        sys.path.insert(0, %r)
        from harbor import files_core as fc
        j = fc.trash_paths(fc.Job(), [%r])
        print("ERR" if j.errors else "OK", j.errors)
    """ % (os.path.join(ROOT, "src"), str(work / "gone.txt")))
    env = dict(os.environ, XDG_DATA_HOME=str(data), HOME=str(tmp_path))
    out = subprocess.run([sys.executable, "-c", code], env=env, capture_output=True, text=True,
                         timeout=30)
    assert "OK" in out.stdout, out.stdout + out.stderr
    assert not (work / "gone.txt").exists()
    assert (data / "Trash" / "files" / "gone.txt").exists()
    info = (data / "Trash" / "info" / "gone.txt.trashinfo").read_text()
    assert "Path=" in info


def test_new_items_unique(tmp_path):
    a = fc.new_item(str(tmp_path), "folder")
    b = fc.new_item(str(tmp_path), "folder")
    t1 = fc.new_item(str(tmp_path), "text")
    t2 = fc.new_item(str(tmp_path), "text")
    assert [os.path.basename(p) for p in (a, b, t1, t2)] == [
        "New folder", "New folder (2)", "New text document.txt", "New text document (2).txt"]
    assert os.path.isdir(b) and os.path.isfile(t2)


def test_rename_path(tmp_path):
    touch(str(tmp_path / "a.txt"))
    touch(str(tmp_path / "b.txt"))
    out = fc.rename_path(str(tmp_path / "a.txt"), "c.txt")
    assert out == str(tmp_path / "c.txt") and os.path.exists(out)
    with pytest.raises(ValueError):
        fc.rename_path(out, "b.txt")
    with pytest.raises(ValueError):
        fc.rename_path(out, "x/y")
    assert fc.rename_path(out, "c.txt") == out


def test_restore_target(tmp_path):
    assert fc.restore_target(str(tmp_path / "f.txt")) == str(tmp_path / "f.txt")
    touch(str(tmp_path / "f.txt"))
    assert fc.restore_target(str(tmp_path / "f.txt")) == str(tmp_path / "f (2).txt")


def test_tree_size(tmp_path):
    touch(str(tmp_path / "a" / "x"), b"123")
    touch(str(tmp_path / "a" / "b" / "y"), b"45")
    assert fc.tree_size(str(tmp_path / "a")) == (5, 2, 1)
    assert fc.tree_size(str(tmp_path / "a" / "x")) == (3, 1, 0)
    assert fc.tree_size(str(tmp_path / "missing")) == (0, 0, 0)


def test_misc_format():
    assert fc.item_count_text(1) == "1 item"
    assert fc.item_count_text(3) == "3 items"
    assert fc.permission_string(0o40755) == "drwxr-xr-x"
    assert fc.format_time(0) == ""
    assert fc.format_time(1700000000)


# ------------------------------------------------------------------ module import (no display)

def test_files_module_imports_and_contract():
    from harbor import files
    assert files.App.__name__ == "App"
    assert callable(files.main)
    assert files.APP_ID == "io.github.hafij_deu.Harbor.Files"


def test_terminal_command_dry_run():
    from harbor import common, files
    common.DRY_RUN_LOG.clear()
    assert files.open_terminal("/tmp", have=lambda p: p == "ptyxis")
    assert common.DRY_RUN_LOG[-1][0] == "ptyxis"
    assert "/tmp" in common.DRY_RUN_LOG[-1][-1]
    assert not files.open_terminal("/tmp", have=lambda p: False)


# ------------------------------------------------------------------ window helpers (pure)

def test_parse_location_keywords_and_uris(tmp_path):
    home = str(tmp_path)
    assert fc.parse_location("", home=home) is None
    assert fc.parse_location("  Recycle Bin ", home=home) == "trash"
    assert fc.parse_location("trash:///", home=home) == "trash"
    assert fc.parse_location("Quick access", home=home) == "home"
    assert fc.parse_location("This computer", home=home) == "/"
    assert fc.parse_location("~/Docs/../Music", home=home) == os.path.join(home, "Music")
    assert fc.parse_location("sub", cwd="/usr", home=home) == "/usr/sub"
    assert fc.parse_location("file:///tmp/a%20b", home=home) == "/tmp/a b"
    assert fc.parse_location("sftp://host/dir", home=home) == "sftp://host/dir"


def test_parse_page_key():
    from harbor.files import parse_page_key
    assert parse_page_key("home") == ("home", None, None)
    assert parse_page_key("folder:/tmp/x") == ("folder", "/tmp/x", None)
    assert parse_page_key("properties:/a:b") == ("properties", "/a:b", None)
    assert parse_page_key("search:/tmp:two") == ("search", "/tmp", "two")
    assert parse_page_key("search:two") == ("search", None, "two")
    assert parse_page_key("bogus") == (None, None, None)


def test_arg_to_location(tmp_path):
    from harbor.files import arg_to_location
    touch(str(tmp_path / "d" / "f.txt"))
    loc, sel = arg_to_location(str(tmp_path / "d" / "f.txt"))
    assert loc == fc.location_for(str(tmp_path / "d")) and sel == str(tmp_path / "d" / "f.txt")
    assert arg_to_location("d", cwd=str(tmp_path)) == (fc.location_for(str(tmp_path / "d")), None)
    assert arg_to_location("trash") == (fc.TRASH_URI, None)
    assert arg_to_location("  ") is None


def test_is_executable_type():
    from harbor.files import is_executable_type
    assert is_executable_type("application/x-executable")
    assert is_executable_type("application/x-desktop")
    assert not is_executable_type("text/plain")
    assert not is_executable_type("")
    assert not is_executable_type(None)


class _Item:
    def __init__(self, name, is_dir=False, size=0, path="/x", caps=("rename", "trash")):
        self.name, self.is_dir, self.size, self.path = name, is_dir, size, path
        self.caps = caps
        self.content_type = "inode/directory" if is_dir else "text/plain"

    def can(self, what):
        return what in self.caps


def test_selection_size_and_sel_dicts():
    from harbor.files import selection_size, sel_dicts
    assert selection_size([]) is None
    assert selection_size([_Item("a", size=3), _Item("b", size=4)]) == 7
    assert selection_size([_Item("a", size=3), _Item("d", is_dir=True)]) is None
    d = sel_dicts([_Item("x.zip"), _Item("dir.zip", is_dir=True), _Item("r", path=None,
                                                                         caps=())])
    assert d[0]["archive"] and d[0]["local"] and d[0]["can_rename"] and d[0]["can_trash"]
    assert not d[1]["archive"] and d[1]["is_dir"]
    assert not d[2]["local"] and not d[2]["can_rename"] and not d[2]["can_trash"]


def test_open_terminal_falls_through_list():
    from harbor import common, files
    common.DRY_RUN_LOG.clear()
    assert files.open_terminal("/tmp/a b", have=lambda p: p == "konsole")
    assert common.DRY_RUN_LOG[-1] == ["konsole", "--workdir", "/tmp/a b"]
    assert fc.terminal_argv("/x", have=lambda p: False) is None
    argv = fc.terminal_argv("/x y", have=lambda p: p == "xterm")
    assert argv[:2] == ["xterm", "-e"] and "'/x y'" in argv[2]


# ------------------------------------------------------------------ archives

@pytest.mark.parametrize("bad", ["/etc/passwd", "../x", "a/../../x", "a\\..\\..\\x", "C:/x",
                                 "a/\0b"])
def test_safe_member_path_refuses(tmp_path, bad):
    with pytest.raises(ValueError):
        fc.safe_member_path(str(tmp_path), bad)


def test_safe_member_path_ok(tmp_path):
    assert fc.safe_member_path(str(tmp_path), "a/./b.txt") == str(tmp_path / "a" / "b.txt")
    assert fc.safe_member_path(str(tmp_path), "dir/") == str(tmp_path / "dir")


def test_zip_slip_refused_and_nothing_written(tmp_path):
    import zipfile
    z = str(tmp_path / "evil.zip")
    with zipfile.ZipFile(z, "w") as zf:
        zf.writestr("ok.txt", "fine")
        zf.writestr("../escape.txt", "bad")
    out = tmp_path / "out"
    out.mkdir()
    with pytest.raises(ValueError):
        fc.extract_archive(fc.Job(), z, str(out))
    assert os.listdir(str(out)) == []
    assert not (tmp_path / "escape.txt").exists()


def test_tar_symlink_escape_refused(tmp_path):
    import io
    import tarfile
    t = str(tmp_path / "evil.tar")
    with tarfile.open(t, "w") as tf:
        info = tarfile.TarInfo("link")
        info.type = tarfile.SYMTYPE
        info.linkname = "../../etc"
        tf.addfile(info)
        data = b"x"
        f = tarfile.TarInfo("ok.txt")
        f.size = len(data)
        tf.addfile(f, io.BytesIO(data))
    out = tmp_path / "out"
    out.mkdir()
    with pytest.raises(ValueError):
        fc.extract_archive(fc.Job(), t, str(out))
    assert os.listdir(str(out)) == []


def test_compress_extract_roundtrip(tmp_path):
    src = tmp_path / "src"
    touch(str(src / "proj" / "a.txt"), b"alpha")
    touch(str(src / "proj" / "sub" / "b.bin"), bytes(range(256)) * 10)
    (src / "proj" / "empty").mkdir()
    touch(str(src / "loose.md"), b"# hi")
    paths = [str(src / "proj"), str(src / "loose.md")]
    assert fc.zip_name_for(paths) == "proj.zip"
    assert fc.zip_name_for([str(src / "loose.md")]) == "loose.zip"
    z = str(tmp_path / "proj.zip")
    job = fc.Job()
    fc.compress_zip(job, paths, z)
    assert job.copied == [z] and not job.errors
    with pytest.raises(FileExistsError):
        fc.compress_zip(fc.Job(), paths, z)        # never overwrites
    out = tmp_path / "out"
    out.mkdir()
    folder = fc.extract_archive(fc.Job(), z, str(out))
    assert folder == str(out / "proj")
    assert read(str(out / "proj" / "proj" / "a.txt")) == b"alpha"
    assert read(str(out / "proj" / "proj" / "sub" / "b.bin")) == bytes(range(256)) * 10
    assert (out / "proj" / "proj" / "empty").is_dir()
    assert read(str(out / "proj" / "loose.md")) == b"# hi"
    # a second extract gets its own folder
    assert fc.extract_archive(fc.Job(), z, str(out)) == str(out / "proj (2)")


def test_archive_kind_and_stem():
    assert fc.archive_kind("a.ZIP") == "zip"
    assert fc.archive_kind("a.tar.gz") == "tar" and fc.archive_kind("a.tzst") == "tar"
    assert fc.archive_kind("a.txt") is None
    assert fc.archive_stem("backup.tar.gz") == "backup"
    assert fc.archive_stem("x.zip") == "x"


# ------------------------------------------------------------------ command rules / status

def _sel(**kw):
    d = dict(is_dir=False, local=True, can_rename=True, can_trash=True, archive=False)
    d.update(kw)
    return d


def test_command_states_rules():
    st = fc.command_states("folder", [], can_paste=True)
    assert st["new"] and st["paste"] and not st["cut"] and not st["rename"] and st["terminal"]
    assert fc.command_states("folder", [], can_paste=True, writable=False)["paste"] is False
    st = fc.command_states("folder", [_sel(), _sel()])
    assert st["cut"] and st["copy"] and st["delete"] and not st["rename"] and st["compress"]
    assert not st["open-with"] and not st["extract"]
    st = fc.command_states("folder", [_sel(archive=True)])
    assert st["rename"] and st["extract"] and st["open-with"]
    st = fc.command_states("folder", [_sel(is_dir=True)])
    assert st["open-new-tab"] and st["pin"] and not st["open-with"]
    st = fc.command_states("trash", [_sel()], n_items=3)
    assert st["restore"] and st["delete"] and st["empty-trash"] and not st["cut"]
    assert not st["open"] and not st["rename"]
    assert not fc.command_states("trash", [], n_items=0)["empty-trash"]
    st = fc.command_states("home", [])
    assert not st["new"] and not st["paste"] and not st["sort"] and not st["select-all"]
    st = fc.command_states("search", [_sel(local=False)])
    assert not st["copy"] and not st["delete"]
    st = fc.command_states("folder", [_sel(can_trash=False)])
    assert not st["cut"]


def test_action_rules_reference_known_states():
    from harbor import files
    keys = set(fc.command_states("folder", []))
    for action, key in files.ACTION_RULES.items():
        assert key in keys, (action, key)


def test_status_text():
    assert fc.status_text(0) == "0 items"
    assert fc.status_text(1) == "1 item"
    assert fc.status_text(12, 2) == "12 items  |  2 items selected"
    t = fc.status_text(12, 1, 2048)
    assert t.startswith("12 items  |  1 item selected  ") and "KB" in t


# ------------------------------------------------------------------ locations

def test_location_helpers(tmp_path):
    home = str(tmp_path)
    assert fc.location_for("home") == "home" and fc.location_for("thispc") == "thispc"
    assert fc.location_for("trash") == fc.TRASH_URI
    assert fc.location_for("/") == "file:///"
    assert fc.location_for("/tmp/a b") == "file:///tmp/a%20b"
    assert fc.location_kind("home") == "home"
    assert fc.location_kind("trash:///") == "trash"
    assert fc.location_kind("file:///tmp") == "folder"
    assert fc.location_kind("smb://srv/share") == "network"
    assert fc.location_title("home") == "Home" and fc.location_title("thispc") == "This PC"
    assert fc.location_title("trash:///") == "Trash"
    assert fc.location_title("network:///") == "Network"
    assert fc.location_title(fc.location_for(home), home=home) == "My files"
    assert fc.location_title("file:///") == "File System"
    assert fc.location_title("file:///tmp/a%20b") == "a b"
    assert fc.parent_location("home") is None
    assert fc.parent_location("file:///tmp/x") == "file:///tmp"
    assert fc.parent_location("file:///") == "thispc"


def test_new_item_kinds(tmp_path):
    e = fc.new_item(str(tmp_path), "empty")
    e2 = fc.new_item(str(tmp_path), "empty")
    t = fc.new_item(str(tmp_path), "bogus")      # unknown kinds fall back to a text document
    assert [os.path.basename(p) for p in (e, e2, t)] == [
        "New file", "New file (2)", "New text document.txt"]
    assert os.path.getsize(e) == 0 and os.path.isfile(t)


def test_settings_validation(tmp_path):
    p = str(tmp_path / "files.json")
    with open(p, "w") as f:
        f.write('{"view": "tiles", "icon_size": "huge", "sort": "colour", "pins": ["/a", 5],'
                ' "show_hidden": "yes", "width": 900, "unknown": 1}')
    s = fc.Settings(p)
    assert s.get("view") == "details" and s.get("icon_size") == "large"
    assert s.get("sort") == "name" and s.pins == ["/a"]
    assert s.get("show_hidden") is False and s.get("width") == 900
    assert "unknown" not in s.data
    s.set("view", "icons")
    assert fc.Settings(p).get("view") == "icons"
    with open(p, "w") as f:
        f.write("[1, 2]")
    assert fc.Settings(p).get("view") == "details"
