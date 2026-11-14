"""Harbor Files: folder colours and emblems for the Modern icon set.

The bundled folder icons (harbor-folder*.svg) are yellow. A colour (the
Folder colour setting, or a per-folder colour from Customize folder) is
applied by rewriting the SVG colours at runtime: a lighter front, a darker
back, the same structure. Emblems (star, heart, lock, ...) are drawn into
the SVG as a small badge in the corner.

Recoloured icons are written once to
$XDG_CACHE_HOME/harbor/folder-icons/<key>/<icon>-<suffix>.svg (key is
"3584e4", "classic-star", "3584e4-lock", ...) in a worker thread; the
directory is then added to the icon theme search path on the main loop.
Until then the plain icon is shown. GTK's dash fallback means an icon name
that is not ready ("harbor-folder-c3584e4") still finds "harbor-folder".
"""

import colorsys
import os
import re
import threading

from gi.repository import GLib

from . import files_core as fc

HEX = re.compile(r"^#[0-9a-fA-F]{6}$")

# original colours of the bundled folder icons
BACK_TOP, BACK_BOTTOM = "#efae38", "#dc9420"
FRONT_TOP, FRONT_BOTTOM = "#ffdc7a", "#f7bd3c"
HIGHLIGHT, SHADE = "#fff3c4", "#c98a12"
CHIP = re.compile(r'(<rect x="22" y="31.6" width="20" height="16.8" rx="4.2" fill="#[0-9a-fA-F]{6}")')

# Customize folder > Change icon: glyph variants of the Harbor folder
FOLDER_GLYPHS = (
    ("harbor-folder", "Plain"), ("harbor-folder-documents", "Documents"),
    ("harbor-folder-downloads", "Downloads"), ("harbor-folder-music", "Music"),
    ("harbor-folder-pictures", "Pictures"), ("harbor-folder-videos", "Videos"),
    ("harbor-folder-desktop", "Desktop"), ("harbor-folder-home", "Home"),
    ("harbor-folder-public", "Public"), ("harbor-folder-templates", "Templates"),
    ("harbor-folder-code", "Code"), ("harbor-folder-games", "Games"),
    ("harbor-folder-work", "Work"), ("harbor-folder-star", "Star"),
    ("harbor-folder-heart", "Heart"), ("harbor-folder-cloud", "Cloud"),
    ("harbor-folder-books", "Books"), ("harbor-folder-archive", "Archive"),
    ("harbor-folder-lock", "Private"), ("harbor-folder-school", "School"),
    ("harbor-folder-camera", "Camera"), ("harbor-folder-apps", "Apps"),
)

# emblem id -> (label, badge colour, white glyph drawn in a 64 px icon)
_ES = 'fill="none" stroke="#fff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"'
EMBLEMS = {
    "star": ("Star", "#e5a50a",
             '<polygon fill="#fff" points="51,43.6 52.6,47.4 56.6,47.7 53.5,50.3 54.5,54.3 '
             '51,52.1 47.5,54.3 48.5,50.3 45.4,47.7 49.4,47.4"/>'),
    "heart": ("Heart", "#e62d42",
              '<path fill="#fff" d="M51 54.6s-4.6-2.7-4.6-5.7a2.4 2.4 0 0 1 4.6-1.1 2.4 2.4 0 0 1 '
              '4.6 1.1c0 3-4.6 5.7-4.6 5.7z"/>'),
    "lock": ("Lock", "#5e5c64",
             '<rect x="47.2" y="48.6" width="7.6" height="5.6" rx="1.1" fill="#fff"/>'
             '<path %s d="M48.9 48.6v-1.5a2.1 2.1 0 0 1 4.2 0v1.5"/>' % _ES),
    "check": ("Done", "#2ec27e", '<path %s d="m47.4 49.4 2.6 2.6 4.6-5"/>' % _ES),
    "important": ("Important", "#ed5b00",
                  '<path %s d="M51 45.6v4.6"/><circle cx="51" cy="53.4" r="1.2" fill="#fff"/>'
                  % _ES),
    "cloud": ("Cloud", "#3584e4",
              '<path fill="#fff" d="M48 53.4h6.4a2.2 2.2 0 0 0 .2-4.4 3 3 0 0 0-5.8-.7 2.6 2.6 0 0 '
              '0-.8 5.1z"/>'),
}

_lock = threading.Lock()
_ready = set()        # keys whose directory is on the icon search path
_pending = set()
_listeners = []       # called on the main loop when a key becomes ready
_source_dirs = None


def valid_hex(value):
    return isinstance(value, str) and bool(HEX.match(value))


def effective_color(setting, custom=None, accent=None, per_folder=None):
    """The colour a folder is drawn in, or None for the classic yellow.

    Precedence: the folder's own colour (Customize folder), then the
    Folder colour setting (accent / palette / custom), then classic.
    """
    if valid_hex(per_folder):
        return per_folder.lower()
    if setting == "accent":
        return accent.lower() if valid_hex(accent) else None
    if setting == "custom":
        return custom.lower() if valid_hex(custom) else None
    if setting in fc.FOLDER_PALETTE:
        return fc.FOLDER_PALETTE[setting]
    return None


# ---------------------------------------------------------------------- colour maths

def _rgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) / 255.0 for i in (0, 2, 4))


def _hex(rgb):
    return "#%02x%02x%02x" % tuple(max(0, min(255, round(c * 255))) for c in rgb)


def shade(h, lightness=None, delta=0.0, sat=None):
    """Same hue with a new (or shifted) lightness / saturation."""
    hh, ll, ss = colorsys.rgb_to_hls(*_rgb(h))
    ll = lightness if lightness is not None else ll + delta
    ss = sat if sat is not None else ss
    return _hex(colorsys.hls_to_rgb(hh, max(0.0, min(1.0, ll)), max(0.0, min(1.0, ss))))


def palette_for(h):
    """{original colour: new colour} for a folder colour."""
    _hh, ll, ss = colorsys.rgb_to_hls(*_rgb(h))
    ss = max(ss, 0.18) if ss > 0.05 else ss     # keep greys grey
    return {
        BACK_TOP: shade(h, max(0.32, min(0.55, ll)), sat=ss),
        BACK_BOTTOM: shade(h, max(0.26, min(0.47, ll - 0.08)), sat=ss),
        FRONT_TOP: shade(h, 0.80, sat=ss),
        FRONT_BOTTOM: shade(h, 0.66, sat=ss),
        HIGHLIGHT: shade(h, 0.92, sat=ss),
        SHADE: shade(h, 0.30, sat=ss),
    }


def recolor_svg(svg, color=None, emblem=None):
    """The folder SVG in a new colour and / or with an emblem badge."""
    if color:
        for old, new in palette_for(color).items():
            svg = re.sub(re.escape(old), new, svg, flags=re.IGNORECASE)
        # the badge chip gets a white outline so it reads on any folder colour
        svg = CHIP.sub(r'\1 stroke="#fff" stroke-width="1.2"', svg)
    if emblem in EMBLEMS:
        _label, col, glyph = EMBLEMS[emblem]
        badge = ('<circle cx="51" cy="50" r="9" fill="%s" stroke="#fff" stroke-width="1.6"/>%s'
                 % (col, glyph))
        svg = svg.rstrip()
        if svg.endswith("</svg>"):
            svg = svg[:-6] + badge + "</svg>\n"
    return svg


# ---------------------------------------------------------------------- names / keys

def key_for(color=None, emblem=None):
    """Cache directory key, or None for the plain icon."""
    if not color and emblem not in EMBLEMS:
        return None
    parts = [color.lstrip("#").lower() if color else "classic"]
    if emblem in EMBLEMS:
        parts.append(emblem)
    return "-".join(parts)


def suffix_for(color=None, emblem=None):
    parts = []
    if color:
        parts.append("c" + color.lstrip("#").lower())
    if emblem in EMBLEMS:
        parts.append("e" + emblem)
    return "-".join(parts)


def is_folder_icon(name):
    return isinstance(name, str) and (name == "harbor-folder" or name.startswith("harbor-folder-"))


def cache_root():
    return os.path.join(GLib.get_user_cache_dir(), "harbor", "folder-icons")


def source_dirs():
    global _source_dirs
    if _source_dirs is None:
        here = os.path.dirname(os.path.realpath(__file__))
        cands = [os.path.join(here, "..", "..", "data", "icons", "hicolor", "scalable", "places"),
                 "/usr/share/harbor/icons/hicolor/scalable/places"]
        _source_dirs = [os.path.normpath(d) for d in cands if os.path.isdir(d)]
    return _source_dirs


def folder_sources():
    """{icon name: svg path} for every bundled folder icon."""
    out = {}
    for d in reversed(source_dirs()):
        try:
            names = os.listdir(d)
        except OSError:
            continue
        for n in names:
            stem, ext = os.path.splitext(n)
            if ext == ".svg" and is_folder_icon(stem):
                out[stem] = os.path.join(d, n)
    return out


def write_variant(key, color, emblem, root=None, sources=None):
    """Worker: write every folder icon for one key; returns the directory."""
    root = root or cache_root()
    out = os.path.join(root, key)
    os.makedirs(out, exist_ok=True)
    suffix = suffix_for(color, emblem)
    for stem, src in (sources or folder_sources()).items():
        dest = os.path.join(out, "%s-%s.svg" % (stem, suffix))
        if os.path.exists(dest):
            continue
        with open(src, encoding="utf-8") as f:
            svg = f.read()
        tmp = dest + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            f.write(recolor_svg(svg, color, emblem))
        os.replace(tmp, dest)
    return out


def icon_name(base, color=None, emblem=None):
    """Name to show for a folder icon; asks for the variant when not ready."""
    if not is_folder_icon(base):
        return base
    key = key_for(color, emblem)
    if key is None:
        return base
    if key in _ready:
        return "%s-%s" % (base, suffix_for(color, emblem))
    request(color, emblem)
    if emblem and color:
        # the colour alone may already be there
        ck = key_for(color, None)
        if ck in _ready:
            return "%s-%s" % (base, suffix_for(color, None))
    return base


def add_listener(cb):
    _listeners.append(cb)


def request(color=None, emblem=None, done=None):
    """Make sure a variant exists (worker thread); done() on the main loop."""
    key = key_for(color, emblem)
    if key is None:
        if done:
            done()
        return
    with _lock:
        if key in _ready:
            if done:
                GLib.idle_add(lambda: (done(), False)[1])
            return
        if key in _pending:
            return
        _pending.add(key)

    def work():
        try:
            path = write_variant(key, color, emblem)
        except OSError:
            path = None
        GLib.idle_add(_finish, key, path, done)
    threading.Thread(target=work, daemon=True, name="hb-tint").start()


def _finish(key, path, done):
    with _lock:
        _pending.discard(key)
    if path:
        try:
            from gi.repository import Gdk, Gtk
            display = Gdk.Display.get_default()
            if display is not None:
                Gtk.IconTheme.get_for_display(display).add_search_path(path)
        except (ImportError, TypeError):
            pass
        _ready.add(key)
        for cb in list(_listeners):
            cb()
    if done:
        done()
    return False


def accent_hex():
    """The GNOME accent colour (libadwaita 1.6+) as #rrggbb, or None."""
    try:
        from gi.repository import Adw
        rgba = Adw.StyleManager.get_default().get_accent_color_rgba()
    except (ImportError, AttributeError, TypeError):
        return None
    return _hex((rgba.red, rgba.green, rgba.blue))
