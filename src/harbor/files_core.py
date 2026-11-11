"""Harbor Files: pure logic (no widgets).

Everything here is testable without a display: names, sorting, paths,
pins, search matching, clipboard formats and the copy/move/delete workers
(synchronous Gio calls meant to run in a worker thread).
"""

import fnmatch
import json
import os
import re
import threading
import urllib.parse

from gi.repository import Gio, GLib

# ------------------------------------------------------------------ names

DOUBLE_EXTS = (".tar.gz", ".tar.xz", ".tar.bz2", ".tar.zst", ".tar.lz", ".tar.lzma")


def split_ext(name):
    """("photo", ".jpg"); dotfiles and folders-like names keep no extension."""
    low = name.lower()
    for ext in DOUBLE_EXTS:
        if low.endswith(ext) and len(name) > len(ext):
            return name[:-len(ext)], name[-len(ext):]
    dot = name.rfind(".")
    if dot <= 0 or dot == len(name) - 1:
        return name, ""
    return name[:dot], name[dot:]


_NUMBERED = re.compile(r"^(.*) \((\d+)\)$")


def unique_name(name, exists, is_dir=False):
    """Return name, or "name (2).ext", "name (3).ext" ... whichever is free.

    exists(candidate) -> bool. A name that already ends in " (N)" counts on
    from N instead of producing "name (2) (2)".
    """
    if not exists(name):
        return name
    stem, ext = (name, "") if is_dir else split_ext(name)
    m = _NUMBERED.match(stem)
    n = 2
    if m:
        stem, n = m.group(1), int(m.group(2)) + 1
    while True:
        cand = "%s (%d)%s" % (stem, n, ext)
        if not exists(cand):
            return cand
        n += 1


def exists_in(folder):
    """exists() callback for unique_name() on a real directory path."""
    return lambda name: os.path.lexists(os.path.join(folder, name))


def validate_name(name, exists=None, old=None):
    """Error text for a bad new file name, or None when it is fine."""
    if name is None or not name.strip():
        return "The name can't be empty."
    if "/" in name or "\0" in name:
        return "A name can't contain \"/\"."
    if name in (".", ".."):
        return "\"%s\" is not a valid name." % name
    if len(name.encode("utf-8", "surrogateescape")) > 255:
        return "The name is too long."
    if exists is not None and name != old and exists(name):
        return "There is already an item named \"%s\" here." % name
    return None


def rename_selection_range(name, is_dir=False):
    """(start, end) of the part to pre-select when renaming (the stem)."""
    if is_dir:
        return 0, len(name)
    stem, _ext = split_ext(name)
    return 0, len(stem)


# ------------------------------------------------------------------ sorting

_DIGITS = re.compile(r"(\d+)")


def natural_key(name):
    """Case-insensitive natural order: "file2" < "file10"."""
    parts = _DIGITS.split(name.casefold())
    return [(0, int(p), "") if p.isdigit() else (1, 0, p) for p in parts if p != ""]


SORT_FIELDS = ("name", "modified", "type", "size")


def sort_key(field, name, is_dir, mtime=0, type_desc="", size=0):
    """Key for one item; folders always sort before files."""
    nk = natural_key(name)
    if field == "modified":
        k = (mtime or 0, nk)
    elif field == "type":
        k = (type_desc.casefold(), nk)
    elif field == "size":
        k = ((0 if is_dir else size or 0), nk)
    else:
        k = (nk,)
    return k


def compare(field, a, b):
    """-1/0/1 for two dicts with name/is_dir/mtime/type/size (ascending)."""
    ka = sort_key(field, a["name"], a["is_dir"], a.get("mtime", 0), a.get("type", ""), a.get("size", 0))
    kb = sort_key(field, b["name"], b["is_dir"], b.get("mtime", 0), b.get("type", ""), b.get("size", 0))
    return (ka > kb) - (ka < kb)


def sort_entries(entries, field="name", descending=False):
    """Folders first (always), then by field. entries: dicts as in compare()."""
    import functools
    dirs = [e for e in entries if e["is_dir"]]
    files = [e for e in entries if not e["is_dir"]]
    cmp = functools.cmp_to_key(lambda a, b: compare(field, a, b))
    dirs.sort(key=cmp, reverse=descending)
    files.sort(key=cmp, reverse=descending)
    return dirs + files


# ------------------------------------------------------------------ paths and locations

def home_dir():
    return GLib.get_home_dir()


def display_name_for(path, home=None):
    home = home or home_dir()
    if path == home:
        return "Home"
    if path == "/":
        return "This computer"
    return os.path.basename(path.rstrip("/")) or path


def breadcrumbs(path, home=None):
    """[(label, path), ...] for the address bar.

    Paths under the home folder start at "Home"; everything else starts at
    "This computer" (/).
    """
    home = (home or home_dir()).rstrip("/") or "/"
    path = os.path.normpath(path) if path else "/"
    if path == home or path.startswith(home + "/"):
        crumbs = [("Home", home)]
        rest = path[len(home):].strip("/")
        base = home
    else:
        crumbs = [("This computer", "/")]
        rest = path.strip("/")
        base = ""
    for part in [p for p in rest.split("/") if p]:
        base = base + "/" + part if base != "/" else "/" + part
        crumbs.append((part, base))
    return crumbs


def parse_location(text, cwd=None, home=None):
    """Turn what the user typed into the address bar into a location key.

    Returns "home", "trash", a filesystem path, or a URI (for other GIO
    schemes) — or None when the text is empty.
    """
    if text is None:
        return None
    t = text.strip()
    if not t:
        return None
    home = home or home_dir()
    low = t.lower()
    if low in ("home", "quick access"):
        return "home"
    if low in ("trash", "trash:", "trash:/", "trash:///", "recycle bin"):
        return "trash"
    if low in ("this computer", "computer"):
        return "/"
    if re.match(r"^[a-zA-Z][a-zA-Z0-9+.-]*://", t) or t.startswith("file:"):
        if low.startswith("file:"):
            g = Gio.File.new_for_uri(t)
            p = g.get_path()
            return os.path.normpath(p) if p else t
        if low.startswith("trash:"):
            return "trash"
        return t
    if t == "~" or t.startswith("~/"):
        t = home + t[1:]
    elif not t.startswith("/"):
        t = os.path.join(cwd or home, t)
    return os.path.normpath(t)


def is_inside(child, parent):
    """True when child equals parent or lives below it (paths)."""
    child = os.path.normpath(child)
    parent = os.path.normpath(parent)
    return child == parent or child.startswith(parent.rstrip("/") + "/")


# ------------------------------------------------------------------ pins (Quick access)

def config_path():
    return os.path.join(GLib.get_user_config_dir(), "harbor", "files.json")


class Settings:
    """Small JSON store: pinned folders and view preferences."""

    DEFAULTS = {"pins": [], "view": "details", "icon_size": "large", "show_hidden": False,
                "sort": "name", "descending": False, "details_pane": False,
                "show_extensions": True, "width": 1200, "height": 760, "maximized": False,
                "hidden_defaults": []}
    LISTS = ("pins", "hidden_defaults")

    def __init__(self, path=None):
        self.path = path or config_path()
        self.data = dict(self.DEFAULTS)
        for k in self.LISTS:
            self.data[k] = []
        self.load()

    def load(self):
        try:
            with open(self.path, encoding="utf-8") as f:
                raw = json.load(f)
        except (OSError, ValueError):
            return
        if not isinstance(raw, dict):
            return
        for k, v in raw.items():
            if k in self.DEFAULTS and isinstance(v, type(self.DEFAULTS[k])):
                self.data[k] = v
        for k in self.LISTS:
            self.data[k] = [p for p in self.data[k] if isinstance(p, str)]
        if self.data["view"] not in ("details", "icons"):
            self.data["view"] = "details"
        if self.data["icon_size"] not in ("large", "medium"):
            self.data["icon_size"] = "large"
        if self.data["sort"] not in SORT_FIELDS:
            self.data["sort"] = "name"

    def save(self):
        os.makedirs(os.path.dirname(self.path), exist_ok=True)
        tmp = self.path + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump(self.data, f, indent=2)
        os.replace(tmp, self.path)

    def get(self, key):
        return self.data.get(key, self.DEFAULTS.get(key))

    def set(self, key, value):
        self.data[key] = value
        self.save()

    @property
    def pins(self):
        return list(self.data["pins"])

    def is_pinned(self, path):
        return os.path.normpath(path) in self.data["pins"]

    def pin(self, path):
        path = os.path.normpath(path)
        if path not in self.data["pins"]:
            self.data["pins"].append(path)
            self.save()
            return True
        return False

    def unpin(self, path):
        path = os.path.normpath(path)
        if path in self.data["pins"]:
            self.data["pins"].remove(path)
            self.save()
            return True
        return False


# ------------------------------------------------------------------ search

def make_matcher(query):
    """Return match(name) for a search box query.

    Plain text is a case-insensitive substring match; text with * or ? is
    a shell wildcard (also case-insensitive). Several words must all match.
    """
    q = (query or "").strip().casefold()
    if not q:
        return lambda name: False
    if any(c in q for c in "*?["):
        pat = q
        return lambda name: fnmatch.fnmatchcase(name.casefold(), pat)
    words = q.split()
    return lambda name: all(w in name.casefold() for w in words)


def search_walk(root, query, cancel=None, show_hidden=False, limit=5000):
    """Yield matching paths below root (recursive, no symlink following).

    cancel: threading.Event (or anything with is_set()). Unreadable
    folders are skipped silently.
    """
    match = make_matcher(query)
    stack = [root]
    found = 0
    while stack:
        if cancel is not None and cancel.is_set():
            return
        folder = stack.pop()
        try:
            it = os.scandir(folder)
        except OSError:
            continue
        with it:
            subdirs = []
            for entry in it:
                if cancel is not None and cancel.is_set():
                    return
                name = entry.name
                if not show_hidden and name.startswith("."):
                    continue
                if match(name):
                    yield entry.path
                    found += 1
                    if found >= limit:
                        return
                try:
                    if entry.is_dir(follow_symlinks=False):
                        subdirs.append(entry.path)
                except OSError:
                    pass
            stack.extend(sorted(subdirs, reverse=True))


# ------------------------------------------------------------------ clipboard formats

def uris_for(paths):
    return [Gio.File.new_for_path(p).get_uri() for p in paths]


def gnome_copied_files(paths, cut):
    """Payload for x-special/gnome-copied-files (Nautilus format)."""
    return ("cut" if cut else "copy") + "\n" + "\n".join(uris_for(paths))


def uri_list(paths):
    return "".join(u + "\r\n" for u in uris_for(paths))


def parse_copied_files(text):
    """(is_cut, [paths]) from x-special/gnome-copied-files or text/uri-list."""
    if not text:
        return False, []
    lines = [ln.strip() for ln in text.replace("\r", "\n").split("\n")]
    lines = [ln for ln in lines if ln and not ln.startswith("#")]
    cut = False
    if lines and lines[0] in ("copy", "cut"):
        cut = lines.pop(0) == "cut"
    paths = []
    for ln in lines:
        if ln.startswith("file:"):
            p = Gio.File.new_for_uri(ln).get_path()
        elif ln.startswith("/"):
            p = ln
        else:
            continue
        if p:
            paths.append(p)
    return cut, paths


# ------------------------------------------------------------------ formatting

def format_time(ts, now=None):
    """Explorer-style date: "11/11/2026 14:05" in the local format."""
    if not ts:
        return ""
    dt = GLib.DateTime.new_from_unix_local(int(ts))
    if dt is None:
        return ""
    return dt.format("%x %H:%M") or ""


def permission_string(mode):
    """'drwxr-xr-x' from st_mode."""
    import stat
    return stat.filemode(mode)


def item_count_text(n):
    return "1 item" if n == 1 else "%d items" % n


# ------------------------------------------------------------------ file operations (worker side)

REPLACE, SKIP, KEEP_BOTH = "replace", "skip", "keep-both"


class Cancelled(Exception):
    pass


class Job:
    """State shared between a worker thread and the UI.

    ask(src_path, dst_path) -> (choice, apply_to_all) is called from the
    worker when a name is taken; the UI implements it (blocking the worker,
    never the main loop). progress(done, total, current_name) is called from
    the worker too; the UI must hop to the main loop itself.
    """

    def __init__(self, ask=None, progress=None):
        self.cancellable = Gio.Cancellable()
        self.ask_cb = ask
        self.progress_cb = progress
        self.sticky = None   # choice applied to all further conflicts
        self.total = 0
        self.done = 0
        self.errors = []
        self.copied = []      # resulting destination paths
        self.skipped = 0

    def cancel(self):
        self.cancellable.cancel()

    @property
    def cancelled(self):
        return self.cancellable.is_cancelled()

    def check(self):
        if self.cancelled:
            raise Cancelled()

    def ask(self, src, dst):
        if self.sticky:
            return self.sticky
        if self.ask_cb is None:
            return KEEP_BOTH
        choice, all_ = self.ask_cb(src, dst)
        if choice is None:
            self.cancel()
            raise Cancelled()
        if all_:
            self.sticky = choice
        return choice

    def progress(self, current):
        if self.progress_cb:
            self.progress_cb(self.done, self.total, current)


def tree_size(path, cancel=None):
    """(bytes, files, folders) below path, no symlink following."""
    size = files = folders = 0
    try:
        st = os.lstat(path)
    except OSError:
        return 0, 0, 0
    if not os.path.isdir(path) or os.path.islink(path):
        return st.st_size, 1, 0
    stack = [path]
    while stack:
        if cancel is not None and cancel.is_set():
            break
        d = stack.pop()
        try:
            it = os.scandir(d)
        except OSError:
            continue
        with it:
            for e in it:
                try:
                    if e.is_dir(follow_symlinks=False):
                        folders += 1
                        stack.append(e.path)
                    else:
                        files += 1
                        size += e.stat(follow_symlinks=False).st_size
                except OSError:
                    pass
    return size, files, folders


def _isdir(path):
    return os.path.isdir(path) and not os.path.islink(path)


def _copy_file(job, src, dst, overwrite):
    flags = Gio.FileCopyFlags.NOFOLLOW_SYMLINKS | Gio.FileCopyFlags.ALL_METADATA
    if overwrite:
        flags |= Gio.FileCopyFlags.OVERWRITE
    base = job.done

    def prog(cur, _tot, *_):
        job.done = base + cur
        job.progress(os.path.basename(src))
    Gio.File.new_for_path(src).copy(Gio.File.new_for_path(dst), flags, job.cancellable, prog, None)
    try:
        job.done = base + os.lstat(src).st_size
    except OSError:
        pass


def _remove_tree(path, cancellable=None):
    if _isdir(path):
        for e in os.scandir(path):
            _remove_tree(e.path, cancellable)
    Gio.File.new_for_path(path).delete(cancellable)


def _transfer(job, src, dest_dir, move, name=None):
    """Copy or move one entry into dest_dir, handling conflicts."""
    job.check()
    name = name or os.path.basename(src.rstrip("/"))
    dst = os.path.join(dest_dir, name)
    merge = False
    overwrite = False
    if os.path.normpath(src) == os.path.normpath(dst):
        if move:
            job.skipped += 1
            return None
        # copy onto itself: always keep both ("name (2)")
        dst = os.path.join(dest_dir, unique_name(name, exists_in(dest_dir), _isdir(src)))
    elif os.path.lexists(dst):
        choice = job.ask(src, dst)
        if choice == SKIP:
            job.skipped += 1
            try:
                job.done += tree_size(src)[0]
            except OSError:
                pass
            return None
        if choice == KEEP_BOTH:
            dst = os.path.join(dest_dir, unique_name(name, exists_in(dest_dir), _isdir(src)))
        elif _isdir(src) and _isdir(dst):
            merge = True
        else:
            if _isdir(dst):
                _remove_tree(dst, job.cancellable)
            else:
                overwrite = True
    if _isdir(src) and is_inside(dst, src):
        raise GLib.Error("Can't put a folder inside itself: " + os.path.basename(src))

    if move and not merge:
        try:
            flags = Gio.FileCopyFlags.NOFOLLOW_SYMLINKS | Gio.FileCopyFlags.NO_FALLBACK_FOR_MOVE
            if overwrite:
                flags |= Gio.FileCopyFlags.OVERWRITE
            Gio.File.new_for_path(src).move(Gio.File.new_for_path(dst), flags, job.cancellable,
                                            None, None)
            job.done += tree_size(dst)[0]
            job.progress(name)
            return dst
        except GLib.Error as e:
            if e.matches(Gio.io_error_quark(), Gio.IOErrorEnum.CANCELLED):
                raise Cancelled()
            # different file system: copy, then delete the source below
    if _isdir(src):
        if not merge:
            os.mkdir(dst)
            try:
                os.chmod(dst, os.stat(src).st_mode & 0o7777)
            except OSError:
                pass
        for e in sorted(os.scandir(src), key=lambda e: e.name):
            _transfer(job, e.path, dst, move)
        if move:
            try:
                os.rmdir(src)
            except OSError:
                pass  # something inside was skipped
    else:
        _copy_file(job, src, dst, overwrite)
        if move:
            Gio.File.new_for_path(src).delete(job.cancellable)
    job.progress(name)
    return dst


def transfer(job, sources, dest_dir, move=False):
    """Copy (or move) sources into dest_dir. Runs synchronously.

    Never overwrites without job.ask() saying REPLACE. Returns job; check
    job.errors / job.copied / job.cancelled afterwards.
    """
    job.total = sum(tree_size(s)[0] for s in sources)
    for src in sources:
        try:
            if _isdir(src) and is_inside(dest_dir, src):
                raise GLib.Error("Can't put a folder inside itself: " + os.path.basename(src))
            out = _transfer(job, src, dest_dir, move)
            if out:
                job.copied.append(out)
        except Cancelled:
            break
        except (GLib.Error, OSError) as e:
            if job.cancelled:
                break
            job.errors.append("%s: %s" % (os.path.basename(src), getattr(e, "message", None) or e))
    return job


def delete_permanently(job, paths):
    job.total = len(paths)
    for p in paths:
        if job.cancelled:
            break
        try:
            _remove_tree(p, job.cancellable)
        except (GLib.Error, OSError) as e:
            if job.cancelled:
                break
            job.errors.append("%s: %s" % (os.path.basename(p), getattr(e, "message", None) or e))
        job.done += 1
        job.progress(os.path.basename(p))
    return job


def trash_paths(job, paths, trash=None):
    """Move paths to the trash (trash(gfile, cancellable) for tests)."""
    trash = trash or (lambda g, c: g.trash(c))
    job.total = len(paths)
    for p in paths:
        if job.cancelled:
            break
        try:
            trash(Gio.File.new_for_path(p), job.cancellable)
            job.copied.append(p)
        except GLib.Error as e:
            job.errors.append("%s: %s" % (os.path.basename(p), e.message))
        job.done += 1
        job.progress(os.path.basename(p))
    return job


NEW_NAMES = {"text": "New text document.txt", "empty": "New file"}


def new_item(folder, kind):
    """Create "New folder" / "New text document.txt" / "New file" with a free
    name (kind folder, text or empty); return the path."""
    if kind == "folder":
        name = unique_name("New folder", exists_in(folder), is_dir=True)
        path = os.path.join(folder, name)
        os.mkdir(path)
    else:
        name = unique_name(NEW_NAMES.get(kind, NEW_NAMES["text"]), exists_in(folder))
        path = os.path.join(folder, name)
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
        os.close(fd)
    return path


def rename_path(path, new_name):
    """Rename in place; refuses bad or existing names (raises ValueError)."""
    folder = os.path.dirname(path)
    old = os.path.basename(path)
    err = validate_name(new_name, exists_in(folder), old)
    if err:
        raise ValueError(err)
    if new_name == old:
        return path
    g = Gio.File.new_for_path(path)
    out = g.set_display_name(new_name, None)
    return out.get_path()


def restore_target(orig_path):
    """Where a trashed item goes back to: orig path, or "name (2)" if taken."""
    folder = os.path.dirname(orig_path)
    name = os.path.basename(orig_path)
    return os.path.join(folder, unique_name(name, exists_in(folder)))


def run_in_thread(fn, done=None):
    """Run fn() in a daemon thread; done(result_or_exc) on the main loop."""
    def worker():
        try:
            res = fn()
        except Exception as e:  # pragma: no cover - surfaced to the UI
            res = e
        if done is not None:
            GLib.idle_add(lambda: (done(res), False)[1])
    t = threading.Thread(target=worker, daemon=True)
    t.start()
    return t


def quote_path(path):
    """Copy-as-path text (quoted like Explorer when it has spaces)."""
    return '"%s"' % path if " " in path else path


def uri_to_label(uri):
    return urllib.parse.unquote(uri)


# ------------------------------------------------------------------ terminal

# (program, argv builder). ptyxis is the GNOME 50 default terminal.
TERMINALS = (
    ("ptyxis", lambda p: ["ptyxis", "--new-window", "--working-directory=" + p]),
    ("gnome-terminal", lambda p: ["gnome-terminal", "--working-directory=" + p]),
    ("kgx", lambda p: ["kgx", "--working-directory=" + p]),
    ("konsole", lambda p: ["konsole", "--workdir", p]),
    ("xfce4-terminal", lambda p: ["xfce4-terminal", "--working-directory=" + p]),
    ("tilix", lambda p: ["tilix", "--working-directory=" + p]),
    ("alacritty", lambda p: ["alacritty", "--working-directory", p]),
    ("kitty", lambda p: ["kitty", "--directory", p]),
    ("foot", lambda p: ["foot", "--working-directory=" + p]),
    ("wezterm", lambda p: ["wezterm", "start", "--cwd", p]),
    ("xterm", lambda p: ["xterm", "-e", "cd " + _sh_quote(p) + " && exec \"${SHELL:-sh}\""]),
)


def _sh_quote(s):
    import shlex
    return shlex.quote(s)


def terminal_argv(path, have):
    """argv that opens a terminal in path, or None when no terminal is installed."""
    for prog, build in TERMINALS:
        if have(prog):
            return build(path)
    return None


# ------------------------------------------------------------------ archives

ARCHIVE_EXTS = (".zip", ".tar", ".tar.gz", ".tgz", ".tar.xz", ".txz", ".tar.bz2", ".tbz2",
                ".tbz", ".tar.zst", ".tzst", ".tar.lzma")


def archive_kind(name):
    """"zip", "tar" or None, from the file name."""
    low = name.lower()
    if low.endswith(".zip"):
        return "zip"
    if any(low.endswith(e) for e in ARCHIVE_EXTS):
        return "tar"
    return None


def archive_stem(name):
    low = name.lower()
    for ext in sorted(ARCHIVE_EXTS, key=len, reverse=True):
        if low.endswith(ext) and len(name) > len(ext):
            return name[:-len(ext)]
    return split_ext(name)[0]


def safe_member_path(dest, member):
    """Where archive member `member` lands below dest.

    Raises ValueError for absolute names, ".." parts or anything else that
    would end up outside dest (zip-slip).
    """
    name = member.replace("\\", "/")
    if not name or name.startswith("/") or re.match(r"^[a-zA-Z]:", name) or "\0" in name:
        raise ValueError("unsafe path in archive: %s" % member)
    parts = [p for p in name.split("/") if p not in ("", ".")]
    if any(p == ".." for p in parts):
        raise ValueError("unsafe path in archive: %s" % member)
    if not parts:
        return os.path.realpath(dest)
    out = os.path.normpath(os.path.join(dest, *parts))
    root = os.path.realpath(dest)
    if not is_inside(os.path.realpath(out), root):
        raise ValueError("unsafe path in archive: %s" % member)
    return out


def _check_tar_member(dest, m):
    safe_member_path(dest, m.name)
    if m.issym() or m.islnk():
        link = m.linkname
        if link.startswith("/"):
            raise ValueError("unsafe link in archive: %s -> %s" % (m.name, link))
        base = os.path.dirname(m.name) if m.issym() else ""
        safe_member_path(dest, os.path.join(base, link) if base else link)


def extract_archive(job, archive, dest_parent):
    """Extract archive into a new folder "<stem>" (unique) in dest_parent.

    Every member is checked before anything is written; one unsafe member
    refuses the whole archive. Returns the new folder (also in job.copied).
    """
    import shutil
    import stat
    import tarfile
    import zipfile
    kind = archive_kind(os.path.basename(archive))
    if kind is None:
        raise ValueError("not an archive")
    folder = os.path.join(dest_parent, unique_name(archive_stem(os.path.basename(archive)),
                                                   exists_in(dest_parent), is_dir=True))
    if kind == "zip":
        with zipfile.ZipFile(archive) as zf:
            infos = zf.infolist()
            for i in infos:
                safe_member_path(folder, i.filename)
            job.total = sum(i.file_size for i in infos) or 1
            os.mkdir(folder)
            try:
                for i in infos:
                    job.check()
                    out = safe_member_path(folder, i.filename)
                    if i.is_dir():
                        os.makedirs(out, exist_ok=True)
                        continue
                    os.makedirs(os.path.dirname(out), exist_ok=True)
                    with zf.open(i) as src, open(out, "xb") as dst:
                        while True:
                            job.check()
                            chunk = src.read(1 << 20)
                            if not chunk:
                                break
                            dst.write(chunk)
                            job.done += len(chunk)
                            job.progress(i.filename)
                    mode = (i.external_attr >> 16) & 0o777
                    if mode and not stat.S_ISLNK(i.external_attr >> 16):
                        os.chmod(out, mode | 0o600)
            except BaseException:
                shutil.rmtree(folder, ignore_errors=True)
                raise
    else:
        with tarfile.open(archive, "r:*") as tf:
            members = tf.getmembers()
            for m in members:
                _check_tar_member(folder, m)
            members = [m for m in members if m.isreg() or m.isdir() or m.issym() or m.islnk()]
            job.total = sum(m.size for m in members) or 1
            os.mkdir(folder)
            try:
                for m in members:
                    job.check()
                    tf.extract(m, folder, set_attrs=True, filter="data")
                    job.done += m.size
                    job.progress(m.name)
            except BaseException:
                shutil.rmtree(folder, ignore_errors=True)
                raise
    job.copied.append(folder)
    return folder


def zip_name_for(paths):
    """Default archive name: first item's name (Explorer style)."""
    first = os.path.basename(paths[0].rstrip("/"))
    stem = first if os.path.isdir(paths[0]) else split_ext(first)[0]
    return (stem or "Archive") + ".zip"


def compress_zip(job, paths, dest_zip):
    """Write paths (files and folders, recursive, symlinks not followed into
    folders) into dest_zip. Removes the partial file on error or cancel."""
    import zipfile
    base = os.path.dirname(os.path.normpath(paths[0]))
    entries = []   # (abs path, arcname, is_dir)
    for p in paths:
        p = os.path.normpath(p)
        top = os.path.relpath(p, base)
        if os.path.isdir(p) and not os.path.islink(p):
            entries.append((p, top, True))
            for d, dirs, files in os.walk(p, followlinks=False):
                dirs.sort()
                rel = os.path.relpath(d, base)
                for name in dirs:
                    full = os.path.join(d, name)
                    if os.path.islink(full):
                        continue
                    entries.append((full, os.path.join(rel, name), True))
                for name in sorted(files):
                    entries.append((os.path.join(d, name), os.path.join(rel, name), False))
        else:
            entries.append((p, top, False))
    dest_real = os.path.realpath(dest_zip)
    entries = [e for e in entries if os.path.realpath(e[0]) != dest_real]
    job.total = 0
    for full, _arc, is_dir in entries:
        if not is_dir:
            try:
                job.total += os.stat(full).st_size
            except OSError:
                pass
    fd = os.open(dest_zip, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    try:
        with os.fdopen(fd, "wb") as raw, zipfile.ZipFile(raw, "w", zipfile.ZIP_DEFLATED) as zf:
            for full, arc, is_dir in entries:
                job.check()
                if is_dir:
                    zf.writestr(zipfile.ZipInfo.from_file(full, arc + "/"), b"")
                    continue
                try:
                    zinfo = zipfile.ZipInfo.from_file(full, arc)
                except OSError as e:
                    job.errors.append("%s: %s" % (arc, e.strerror or e))
                    continue
                zinfo.compress_type = zipfile.ZIP_DEFLATED
                with open(full, "rb") as src, zf.open(zinfo, "w") as dst:
                    while True:
                        job.check()
                        chunk = src.read(1 << 20)
                        if not chunk:
                            break
                        dst.write(chunk)
                        job.done += len(chunk)
                        job.progress(arc)
    except BaseException:
        try:
            os.unlink(dest_zip)
        except OSError:
            pass
        raise
    job.copied.append(dest_zip)
    return dest_zip


# ------------------------------------------------------------------ command bar rules

def command_states(kind, sel, can_paste=False, writable=True, n_items=0):
    """Which commands are enabled.

    kind: home, thispc, folder, search, trash, network (any other non-path
    location). sel: one dict per selected item with keys is_dir, local
    (has a path), can_rename, can_trash, archive (bool).
    """
    n = len(sel)
    listing = kind in ("folder", "search", "trash", "network")
    files = kind in ("folder", "search")
    local = n > 0 and all(s.get("local") for s in sel)
    one = sel[0] if n == 1 else {}
    st = {
        "new": kind == "folder" and writable,
        "cut": files and local and all(s.get("can_trash", True) for s in sel),
        "copy": files and local,
        "paste": kind == "folder" and writable and can_paste,
        "rename": files and n == 1 and bool(one.get("can_rename", True)),
        "delete": (files and local) or (kind == "trash" and n > 0),
        "delete-permanent": (files and local) or (kind == "trash" and n > 0),
        "restore": kind == "trash" and n > 0,
        "empty-trash": kind == "trash" and n_items > 0,
        "open": n > 0 and kind != "trash",
        "open-new-tab": n > 0 and kind != "trash" and all(s.get("is_dir") for s in sel),
        "open-with": n == 1 and kind != "trash" and not one.get("is_dir"),
        "copy-path": (n > 0 and local) or (n == 0 and kind == "folder"),
        "compress": files and local,
        "extract": files and n == 1 and local and bool(one.get("archive")),
        "pin": (n == 1 and bool(one.get("is_dir")) and bool(one.get("local"))
                and kind != "trash") or (n == 0 and kind == "folder"),
        "terminal": kind == "folder" or (kind == "search" and n == 1 and bool(one.get("is_dir"))
                                         and bool(one.get("local"))),
        "properties": n > 0 or kind in ("folder", "trash"),
        "select-all": listing,
        "select-none": listing and n > 0,
        "invert-selection": listing,
        "sort": listing,
        "view": listing,
    }
    return st


def status_text(n_items, n_selected=0, selected_size=None):
    """Status bar text: "12 items  |  2 items selected  4.0 KB"."""
    from .common import human_size
    text = item_count_text(n_items)
    if n_selected:
        text += "  |  %s selected" % item_count_text(n_selected)
        if selected_size is not None:
            text += "  " + human_size(selected_size)
    return text


# ------------------------------------------------------------------ locations

HOME_PAGE, THIS_PC = "home", "thispc"
TRASH_URI, NETWORK_URI = "trash:///", "network:///"


def location_for(key):
    """Normalise parse_location()/show_page keys to a tab location:
    "home", "thispc" or a URI."""
    if key in (HOME_PAGE, THIS_PC):
        return key
    if key == "trash":
        return TRASH_URI
    if key == "/":
        return Gio.File.new_for_path("/").get_uri()
    if key.startswith("/"):
        return Gio.File.new_for_path(key).get_uri()
    return key


def location_kind(loc):
    if loc in (HOME_PAGE, THIS_PC):
        return loc
    if loc.startswith("trash:"):
        return "trash"
    if loc.startswith("file:"):
        return "folder"
    return "network"


def location_title(loc, home=None, path=None):
    """Tab title for a location ("Home", "This PC", "Documents", ...)."""
    home = home or home_dir()
    if loc == HOME_PAGE:
        return "Home"
    if loc == THIS_PC:
        return "This PC"
    if loc.startswith("trash:"):
        return "Trash"
    if loc.rstrip("/") in ("network:", "network:/"):
        return "Network"
    if path is None and loc.startswith("file:"):
        path = Gio.File.new_for_uri(loc).get_path()
    if path:
        path = os.path.normpath(path)
        if path == os.path.normpath(home):
            return "My files"
        if path == "/":
            return "File System"
        return os.path.basename(path)
    g = Gio.File.new_for_uri(loc)
    base = g.get_basename()
    if base and base != "/":
        return urllib.parse.unquote(base)
    return urllib.parse.unquote(loc)


def parent_location(loc):
    """Location for the Up button, or None."""
    if loc in (HOME_PAGE, THIS_PC):
        return None
    g = Gio.File.new_for_uri(loc)
    p = g.get_parent()
    if p is None:
        return THIS_PC if loc.startswith("file:") else None
    return p.get_uri()
