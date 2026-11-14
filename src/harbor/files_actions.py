"""Harbor Files: 7-Zip commands and the extra context-menu commands.

Every function takes the FilesWindow. Long work runs through win.ops._start
(a worker thread with progress and a toast on error); dialogs are Adw.
"""

import hashlib
import os
import shutil
import stat

from gi.repository import Adw, Gio, GLib, Gtk

from . import common
from . import files_archive as fa
from . import files_core as fc

FORMATS = ("7z", "zip", "tar", "tar.gz", "tar.xz", "tar.zst")
LEVELS = (("Store", 0), ("Fastest", 1), ("Normal", 5), ("Maximum", 7), ("Ultra", 9))
TEMPLATES = {"markdown": ("New document.md", None),
             "docx": ("New document.docx", "blank.docx"),
             "xlsx": ("New spreadsheet.xlsx", "blank.xlsx"),
             "pptx": ("New presentation.pptx", "blank.pptx")}


def template_dirs():
    here = os.path.dirname(os.path.abspath(__file__))
    return (os.path.normpath(os.path.join(here, "..", "..", "data", "templates")),
            "/usr/share/harbor/templates")


def template_path(name):
    for d in template_dirs():
        p = os.path.join(d, name)
        if os.path.isfile(p):
            return p
    return None


def new_document(folder, kind):
    """Create a blank Markdown / docx / xlsx / pptx with a free name; return its path."""
    base, tpl = TEMPLATES[kind]
    name = fc.unique_name(base, fc.exists_in(folder))
    path = os.path.join(folder, name)
    src = template_path(tpl) if tpl else None
    if tpl and src is None:
        raise OSError("the %s template is missing" % kind)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    with os.fdopen(fd, "wb") as out:
        if src:
            with open(src, "rb") as f:
                shutil.copyfileobj(f, out)
    return path


def duplicate_name(path, taken):
    """"name - Copy.ext", then "name - Copy (2).ext"..."""
    folder, base = os.path.split(path)
    stem, ext = (base, "") if os.path.isdir(path) else os.path.splitext(base)
    if stem.startswith(".") and not ext:
        stem, ext = base, ""
    cand = "%s - Copy%s" % (stem, ext)
    n = 2
    while cand in taken:
        cand = "%s - Copy (%d)%s" % (stem, n, ext)
        n += 1
    return os.path.join(folder, cand)


def make_executable_mode(mode):
    """Add x wherever r is set (like chmod +x respecting the umask-ish read bits)."""
    return mode | ((mode & 0o444) >> 2)


# ---------------------------------------------------------------- helpers
def _paths(win):
    tab, items = win._sel()
    return tab, [i.path for i in items if i.path]


def _exe(win):
    exe = fa.find_7z()
    if exe is None:
        win.notify_toast("7-Zip is not installed (sudo apt install 7zip)", timeout=6)
    return exe


def _select_after(tab, folder):
    def done(j):
        if j.copied and tab.folder_path() == folder:
            tab.view.select_paths([p for p in j.copied if os.path.dirname(p) == folder])
    return done


def _ask_password(win, archive, then):
    dlg = Adw.AlertDialog(heading="Password needed",
                          body="“%s” is encrypted." % os.path.basename(archive))
    row = Adw.PasswordEntryRow(title="Password")
    box = Gtk.ListBox(css_classes=["boxed-list"], selection_mode=Gtk.SelectionMode.NONE)
    box.append(row)
    dlg.set_extra_child(box)
    dlg.add_response("cancel", "Cancel")
    dlg.add_response("ok", "OK")
    dlg.set_default_response("ok")
    dlg.set_response_appearance("ok", Adw.ResponseAppearance.SUGGESTED)
    row.connect("entry-activated", lambda *_: dlg.response("ok"))

    def resp(_d, r):
        if r == "ok" and row.get_text():
            then(row.get_text())
    dlg.connect("response", resp)
    dlg.present(win)


# ---------------------------------------------------------------- 7-Zip
def extract_into(win, archive, dest, password=None, open_after=False):
    exe = _exe(win)
    if exe is None:
        return
    tab = win.tab()
    job = fc.Job()

    def work(j):
        try:
            return fa.extract(j, exe, archive, dest, password, collision="rename")
        except fa.PasswordNeeded:
            j.need_password = True
            return None

    def done(j):
        if getattr(j, "need_password", False):
            _ask_password(win, archive, lambda pw: extract_into(win, archive, dest, pw,
                                                                open_after))
            return
        if j.errors:
            return
        if open_after:
            win.open_location(fc.location_for(dest))
        elif tab is not None and tab.folder_path() == os.path.dirname(dest):
            tab.view.select_paths([dest])
    win.ops._start("Extracting “%s”" % os.path.basename(archive), work, job, done)


def archive_cache_dir(archive):
    st = os.stat(archive)
    key = hashlib.sha1(("%s\0%d\0%d" % (archive, st.st_mtime_ns, st.st_size))
                       .encode("utf-8", "surrogateescape")).hexdigest()
    return os.path.join(GLib.get_user_cache_dir(), "harbor", "archives", key)


def sevenzip(win, what):
    """win.7z action: open, extract-dialog, extract-here, extract-to, test,
    add-dialog, add-7z, add-zip, hash."""
    tab, paths = _paths(win)
    if not paths:
        return
    first = paths[0]
    folder = os.path.dirname(os.path.normpath(first))
    if what == "open":
        dest = archive_cache_dir(first)
        if os.path.isdir(dest) and os.listdir(dest):
            win.open_location(fc.location_for(dest))
        else:
            extract_into(win, first, dest, open_after=True)
    elif what == "extract-here":
        extract_into(win, first, folder)
    elif what == "extract-to":
        stem = fa.archive_stem(os.path.basename(first))
        dest = os.path.join(folder, fc.unique_name(stem, fc.exists_in(folder), is_dir=True))
        extract_into(win, first, dest)
    elif what == "extract-dialog":
        fd = Gtk.FileDialog(title="Extract to", accept_label="Extract")
        fd.set_initial_folder(Gio.File.new_for_path(folder))

        def picked(d, res):
            try:
                g = d.select_folder_finish(res)
            except GLib.Error:
                return
            if g is not None and g.get_path():
                extract_into(win, first, g.get_path())
        fd.select_folder(win, None, picked)
    elif what == "test":
        test_archive(win, first)
    elif what in ("add-7z", "add-zip"):
        fmt = what[4:]
        stem = os.path.basename(os.path.normpath(first)) if len(paths) == 1 else \
            os.path.basename(folder) or "Archive"
        if len(paths) == 1 and not os.path.isdir(first):
            stem = os.path.splitext(stem)[0]
        add_archive(win, paths, os.path.join(folder, fc.unique_name(
            "%s.%s" % (stem, fmt), fc.exists_in(folder))), fmt)
    elif what == "add-dialog":
        add_dialog(win, paths)
    elif what == "hash":
        hash_dialog(win, paths)


def test_archive(win, archive, password=None):
    exe = _exe(win)
    if exe is None:
        return
    job = fc.Job()
    res = {}

    def work(j):
        try:
            res["ok"], res["msg"] = fa.test(j, exe, archive, password)
        except fa.PasswordNeeded:
            j.need_password = True

    def done(j):
        if getattr(j, "need_password", False):
            _ask_password(win, archive, lambda pw: test_archive(win, archive, pw))
        elif "ok" in res:
            name = os.path.basename(archive)
            win.notify_toast("“%s”: no errors" % name if res["ok"] else
                             "“%s”: %s" % (name, res["msg"] or "errors found"), timeout=6)
    win.ops._start("Testing “%s”" % os.path.basename(archive), work, job, done)


def add_archive(win, paths, dest, fmt="7z", level=5, password=None, encrypt_names=False,
                delete_after=False):
    exe = fa.find_7z()
    if exe is None and fmt not in ("tar.gz", "tar.xz", "tar.zst"):
        _exe(win)
        return
    tab = win.tab()
    job = fc.Job()
    win.ops._start("Compressing to “%s”" % os.path.basename(dest),
                   lambda j: fa.add(j, exe, paths, dest, fmt, level, password, encrypt_names,
                                    "", delete_after),
                   job, _select_after(tab, os.path.dirname(dest)))


def add_dialog(win, paths):
    folder = os.path.dirname(os.path.normpath(paths[0]))
    base = os.path.basename(os.path.normpath(paths[0])) if len(paths) == 1 else \
        (os.path.basename(folder) or "Archive")
    if len(paths) == 1 and not os.path.isdir(paths[0]):
        base = os.path.splitext(base)[0]
    dlg = Adw.AlertDialog(heading="Add to archive",
                          body=fc.item_count_text(len(paths)))
    box = Gtk.ListBox(css_classes=["boxed-list"], selection_mode=Gtk.SelectionMode.NONE)
    name = Adw.EntryRow(title="Archive name", text=base)
    fmt = Adw.ComboRow(title="Format", model=Gtk.StringList.new(list(FORMATS)))
    level = Adw.ComboRow(title="Compression level",
                         model=Gtk.StringList.new([lab for lab, _ in LEVELS]), selected=2)
    pw = Adw.PasswordEntryRow(title="Password (7z / zip only)")
    enc = Adw.SwitchRow(title="Encrypt file names", subtitle="7z only")
    delete = Adw.SwitchRow(title="Delete files after compressing")
    for r in (name, fmt, level, pw, enc, delete):
        box.append(r)
    dlg.set_extra_child(box)
    dlg.add_response("cancel", "Cancel")
    dlg.add_response("ok", "Compress")
    dlg.set_default_response("ok")
    dlg.set_response_appearance("ok", Adw.ResponseAppearance.SUGGESTED)

    def resp(_d, r):
        if r != "ok":
            return
        f = FORMATS[fmt.get_selected()]
        n = (name.get_text().strip() or base).replace("/", "_")
        if not n.lower().endswith("." + f):
            n += "." + f
        dest = os.path.join(folder, fc.unique_name(n, fc.exists_in(folder)))
        password = pw.get_text() or None
        if password and f not in ("7z", "zip"):
            win.notify_toast("Passwords need the 7z or zip format", timeout=5)
            return
        add_archive(win, paths, dest, f, LEVELS[level.get_selected()][1], password,
                    enc.get_active() and f == "7z", delete.get_active())
    dlg.connect("response", resp)
    dlg.present(win)


def hash_dialog(win, paths):
    exe = _exe(win)
    if exe is None:
        return
    files = [p for p in paths if os.path.isfile(p)]
    if not files:
        win.notify_toast("Select files to check")
        return

    def done(rows):
        if isinstance(rows, Exception) or not rows:
            win.notify_toast("Couldn't compute checksums: %s" % rows, timeout=6)
            return
        text = "\n\n".join("%s\nCRC32   %s\nSHA-256 %s" % r for r in rows)
        dlg = Adw.AlertDialog(heading="CRC / SHA checksums")
        lab = Gtk.Label(label=text, selectable=True, wrap=True, xalign=0,
                        css_classes=["monospace"])
        lab.set_wrap_mode(2)
        dlg.set_extra_child(lab)
        dlg.add_response("copy", "Copy")
        dlg.add_response("close", "Close")
        dlg.set_default_response("close")
        dlg.connect("response", lambda _d, r: r == "copy" and win._copy_text(text))
        dlg.present(win)
    win.notify_toast("Computing checksums…")
    fc.run_in_thread(lambda: fa.hashes(exe, files), done)


# ---------------------------------------------------------------- extra commands
def duplicate(win):
    tab, paths = _paths(win)
    if not paths:
        return
    folder = os.path.dirname(os.path.normpath(paths[0]))
    job = fc.Job()

    def work(j):
        names = set(os.listdir(folder))
        for p in paths:
            if j.cancelled:
                break
            dest = duplicate_name(p, names)
            names.add(os.path.basename(dest))
            if common.DRY_RUN:
                common.DRY_RUN_LOG.append(["duplicate", p, dest])
                continue
            try:
                if os.path.isdir(p) and not os.path.islink(p):
                    shutil.copytree(p, dest, symlinks=True)
                else:
                    shutil.copy2(p, dest, follow_symlinks=False)
                j.copied.append(dest)
            except OSError as e:
                j.errors.append("%s: %s" % (os.path.basename(p), e.strerror or e))
    win.ops._start("Duplicating %s" % fc.item_count_text(len(paths)), work, job,
                   _select_after(tab, folder))


def copy_name(win):
    _tab, items = win._sel()
    if items:
        win._copy_text("\n".join(i.name for i in items))


def make_executable(win):
    _tab, paths = _paths(win)
    done = 0
    for p in paths:
        if os.path.isdir(p):
            continue
        try:
            mode = os.stat(p).st_mode
            if not common.DRY_RUN:
                os.chmod(p, stat.S_IMODE(make_executable_mode(mode)))
            done += 1
        except OSError as e:
            win.notify_toast("%s: %s" % (os.path.basename(p), e.strerror or e))
            return
    if done:
        win.notify_toast("Made %s executable" % fc.item_count_text(done))


def set_wallpaper(win):
    _tab, paths = _paths(win)
    if len(paths) != 1:
        return
    uri = Gio.File.new_for_path(paths[0]).get_uri()
    if common.DRY_RUN:
        common.DRY_RUN_LOG.append(["wallpaper", uri])
        win.notify_toast("Wallpaper set (dry run)")
        return
    try:
        bg = Gio.Settings.new("org.gnome.desktop.background")
        bg.set_string("picture-uri", uri)
        bg.set_string("picture-uri-dark", uri)
        win.notify_toast("Wallpaper set")
    except (GLib.Error, TypeError) as e:
        win.notify_toast("Couldn't set the wallpaper: %s" % e)


def new_doc(win, kind):
    tab = win.tab()
    folder = tab.folder_path() if tab else None
    if not folder or tab.kind() != "folder" or kind not in TEMPLATES:
        return
    try:
        path = new_document(folder, kind)
    except OSError as e:
        win.notify_toast("Can't create the document: %s" % (e.strerror or e), timeout=5)
        return
    win.rename_when_visible(tab, path, False)
