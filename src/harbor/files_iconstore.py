"""Harbor Files: where a custom folder icon lives, and how it is kept safe.

Two places, picked in Customize folder > "Show this icon in":

* Harbor Files only: a private JSON store (~/.config/harbor/folder-icons.json,
  mode 0600). Other file managers never see it. It wins over the global
  choice inside Harbor.
* All file managers: GIO metadata (metadata::custom-icon / custom-icon-name),
  the keys Files (Nautilus), Nemo, Caja and the desktop icons read. Harbor's
  own glyphs and colours are not in any icon theme, so they are exported to a
  standalone SVG first and the metadata points at that file.

Every change saves what was there before, so "Undo last change" can put it
back (10 steps per folder, per place).

Safety:
* Pictures are never used in place. They are decoded with GdkPixbuf (size
  and type checked first) and re-encoded as a 256 px PNG in Harbor's icon
  folder, so SVG scripts or external references, odd ICO/WebP payloads and
  metadata are dropped, and later edits to the original don't matter.
* Icon references read back (from our store or from metadata other apps
  wrote) must be local regular files under 5 MB with a picture extension;
  anything else (http://, sftp://, devices, FIFOs, huge files) is ignored, so
  browsing a folder never fetches from the network or blocks on a pipe.
* Icon names must look like theme names; colours must be #rrggbb; badges
  must be known keys.
"""

import hashlib
import json
import os
import re
import stat
import tempfile

from gi.repository import GLib

from . import files_tint as tint

VERSION = 1
HISTORY_MAX = 10
PICTURE_EXTS = (".png", ".svg", ".jpg", ".jpeg", ".webp", ".ico", ".gif", ".bmp", ".avif", ".jxl")
STORED_EXTS = (".png", ".svg")          # what Harbor itself writes
PICTURE_MAX = 5 * 1024 * 1024
ICON_PX = 256
SCOPES = ("harbor", "global")

_NAME_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._+-]{0,127}$")
FIELDS = ("icon_uri", "icon_name", "color", "emblem")


# ------------------------------------------------------------------ paths

def icons_dir():
    """Harbor's own copies of pictures and exported glyphs."""
    return os.path.join(GLib.get_user_data_dir(), "harbor", "folder-icons")


def store_path():
    return os.path.join(GLib.get_user_config_dir(), "harbor", "folder-icons.json")


def key_for(path):
    """Store key for a folder: its absolute, normalised local path."""
    if not path or not os.path.isabs(path):
        return None
    return os.path.normpath(path)


# ------------------------------------------------------------------ validation

def valid_name(name):
    return isinstance(name, str) and bool(_NAME_RE.match(name))


def safe_icon_path(ref, exts=PICTURE_EXTS):
    """Local picture path for an icon reference, or None when it is unsafe.

    ref may be a file:// URI or an absolute path. Remote URIs, relative paths,
    non-regular files, files over 5 MB and non-picture extensions are refused.
    """
    if not isinstance(ref, str) or not ref or "\x00" in ref:
        return None
    if "://" in ref:
        if not ref.startswith("file://"):
            return None
        try:
            path, _host = GLib.filename_from_uri(ref)
        except GLib.Error:
            return None
    else:
        path = ref
    if not path or not os.path.isabs(path):
        return None
    if os.path.splitext(path)[1].lower() not in exts:
        return None
    try:
        st = os.stat(path)          # follows a symlink; the target must qualify
    except (OSError, ValueError):
        return None
    if not stat.S_ISREG(st.st_mode) or st.st_size > PICTURE_MAX or st.st_size == 0:
        return None
    return path


def clean_state(d):
    """Sanitised {icon_uri, icon_name, color, emblem} or None when empty."""
    if not isinstance(d, dict):
        return None
    out = {}
    uri = d.get("icon_uri")
    if uri and safe_icon_path(uri):
        out["icon_uri"] = uri
    name = d.get("icon_name")
    if valid_name(name):
        out["icon_name"] = name
    color = d.get("color")
    if tint.valid_hex(color):
        out["color"] = color.lower()
    emblem = d.get("emblem")
    if emblem in tint.EMBLEMS:
        out["emblem"] = emblem
    if "icon_uri" in out:
        out.pop("icon_name", None)      # a picture wins over a theme name
    return out or None


# ------------------------------------------------------------------ store

class Store:
    """Harbor-only icons plus the undo history of both places."""

    def __init__(self, path=None):
        self.path = path or store_path()
        self.items = {}
        self.history = {}
        self._mtime = None
        self.load()

    def load(self):
        try:
            st = os.stat(self.path)
            if not stat.S_ISREG(st.st_mode) or st.st_size > 4 * 1024 * 1024:
                raise ValueError("not a sane store")
            with open(self.path, encoding="utf-8") as f:
                data = json.load(f)
            self._mtime = st.st_mtime_ns
        except (OSError, ValueError):
            data = {}
        items = data.get("items") if isinstance(data, dict) else None
        hist = data.get("history") if isinstance(data, dict) else None
        self.items = {}
        for k, v in (items or {}).items():
            key = key_for(k) if isinstance(k, str) else None
            st = clean_state(v)
            if key and st:
                self.items[key] = st
        self.history = {}
        for k, steps in (hist or {}).items():
            key = key_for(k) if isinstance(k, str) else None
            if not key or not isinstance(steps, list):
                continue
            good = []
            for s in steps[-HISTORY_MAX:]:
                if isinstance(s, dict) and s.get("scope") in SCOPES:
                    good.append({"scope": s["scope"], "state": clean_state(s.get("state"))})
            if good:
                self.history[key] = good

    def reload_if_changed(self):
        try:
            m = os.stat(self.path).st_mtime_ns
        except OSError:
            m = None
        if m != self._mtime:
            self.load()

    def save(self):
        d = os.path.dirname(self.path)
        os.makedirs(d, mode=0o700, exist_ok=True)
        data = {"version": VERSION, "items": self.items, "history": self.history}
        fd, tmp = tempfile.mkstemp(prefix=".folder-icons.", dir=d)
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                json.dump(data, f, indent=1, sort_keys=True)
            os.chmod(tmp, 0o600)
            os.replace(tmp, self.path)
        except BaseException:
            try:
                os.unlink(tmp)
            except OSError:
                pass
            raise
        try:
            self._mtime = os.stat(self.path).st_mtime_ns
        except OSError:
            self._mtime = None

    # Harbor-only icons
    def get(self, path):
        key = key_for(path)
        st = self.items.get(key) if key else None
        return clean_state(st) if st else None    # re-check: the picture may be gone

    def put(self, path, state):
        key = key_for(path)
        if not key:
            raise ValueError("Only local folders can have a Harbor-only icon")
        st = clean_state(state)
        if st:
            self.items[key] = st
        else:
            self.items.pop(key, None)

    # undo history
    def push(self, path, scope, previous):
        key = key_for(path)
        if not key or scope not in SCOPES:
            return
        steps = self.history.setdefault(key, [])
        steps.append({"scope": scope, "state": clean_state(previous)})
        del steps[:-HISTORY_MAX]

    def can_undo(self, path):
        key = key_for(path)
        return bool(key and self.history.get(key))

    def pop(self, path):
        """Last saved step {"scope", "state"} (removed), or None."""
        key = key_for(path)
        steps = self.history.get(key) if key else None
        if not steps:
            return None
        step = steps.pop()
        if not steps:
            self.history.pop(key, None)
        return step


_store = None


_checked = 0.0


def store():
    """Shared store, reloaded when another Harbor process saved it.

    Called for every folder in a listing, so the file is checked at most
    once a second.
    """
    global _store, _checked
    import time
    now = time.monotonic()
    if _store is None:
        _store = Store()
        _checked = now
    elif now - _checked > 1.0:
        _checked = now
        _store.reload_if_changed()
    return _store


def reset_store_for_tests(path=None):
    global _store
    _store = Store(path) if path else None
    return _store


# ------------------------------------------------------------------ pictures

def _write_unique(data, ext, root):
    """Write bytes to root/<sha256>.<ext> (content-addressed); return the path."""
    os.makedirs(root, mode=0o755, exist_ok=True)
    dest = os.path.join(root, hashlib.sha256(data).hexdigest() + ext)
    if not os.path.isfile(dest):
        fd, tmp = tempfile.mkstemp(prefix=".icon.", dir=root)
        try:
            with os.fdopen(fd, "wb") as f:
                f.write(data)
            os.chmod(tmp, 0o644)        # other file managers (same user) read it
            os.replace(tmp, dest)
        except BaseException:
            try:
                os.unlink(tmp)
            except OSError:
                pass
            raise
    return dest


def import_picture(src, root=None):
    """Decode a picture and keep a clean 256 px PNG copy; return its path.

    Raises ValueError with a message for the user.
    """
    from gi.repository import GdkPixbuf
    if not isinstance(src, str) or not os.path.isabs(src):
        raise ValueError("Pick a picture on this computer")
    ext = os.path.splitext(src)[1].lower()
    if ext not in PICTURE_EXTS:
        raise ValueError("Use a PNG, SVG, JPG, WebP, ICO, GIF, BMP, AVIF or JPEG XL picture")
    try:
        st = os.stat(src)
    except OSError:
        raise ValueError("Harbor Files can't read that picture")
    if not stat.S_ISREG(st.st_mode):
        raise ValueError("That is not a picture file")
    if st.st_size == 0 or st.st_size > PICTURE_MAX:
        raise ValueError("The picture must be smaller than 5 MB")
    # Read once, within the limit, and decode from memory: what is checked is
    # exactly what is decoded (no swap between the check and the read).
    with open(src, "rb") as f:
        raw = f.read(PICTURE_MAX + 1)
    if len(raw) > PICTURE_MAX:
        raise ValueError("The picture must be smaller than 5 MB")
    try:
        loader = GdkPixbuf.PixbufLoader()

        def size_prepared(ld, w, h):
            if w <= 0 or h <= 0 or w > 16384 or h > 16384:
                return
            s = min(1.0, ICON_PX / max(w, h))
            ld.set_size(max(1, round(w * s)), max(1, round(h * s)))
        loader.connect("size-prepared", size_prepared)
        loader.write(raw)
        loader.close()
        pix = loader.get_pixbuf()
    except GLib.Error:
        pix = None
    if pix is None:
        raise ValueError("Harbor Files can't read that picture")
    try:
        ok, png = pix.save_to_bufferv("png", [], [])
    except GLib.Error:
        ok = False
    if not ok:
        raise ValueError("Harbor Files can't convert that picture")
    return _write_unique(bytes(png), ".png", root or icons_dir())


def export_glyph(icon_name, color=None, emblem=None, root=None):
    """Standalone SVG of a Harbor folder glyph in a colour, for other apps.

    Returns the path, or None when icon_name is not a bundled Harbor folder
    icon (theme icons are passed by name instead).
    """
    if not tint.is_folder_icon(icon_name or ""):
        return None
    src = tint.folder_sources().get(icon_name)
    if not src:
        return None
    with open(src, encoding="utf-8") as f:
        svg = f.read()
    out = tint.recolor_svg(svg, color if tint.valid_hex(color) else None,
                           emblem if emblem in tint.EMBLEMS else None)
    return _write_unique(out.encode("utf-8"), ".svg", root or icons_dir())


def global_values(state, default_glyph="harbor-folder"):
    """(custom-icon uri, custom-icon-name) to write for other file managers.

    A picture is used as is. A Harbor glyph, colour or badge becomes an
    exported SVG. A theme icon name is passed by name (no colour: other apps
    can't tint it).
    """
    st = clean_state(state) or {}
    if st.get("icon_uri"):
        return st["icon_uri"], None
    name = st.get("icon_name")
    color, emblem = st.get("color"), st.get("emblem")
    if name and not tint.is_folder_icon(name):
        return None, name
    if name or color or emblem:
        path = export_glyph(name or default_glyph, color, emblem)
        if path:
            return GLib.filename_to_uri(path, None), None
    return None, None
