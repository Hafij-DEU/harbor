"""Harbor Files: bundled icon set and the one content-type -> icon table.

The icons are Harbor's own SVGs (data/icons/hicolor/scalable/..., installed
to /usr/share/harbor/icons). They all have a "harbor-" prefix so they never
clash with an icon theme. Settings > Look > Icons switches between them
("modern") and the system icon theme ("system"). Thumbnails always win over
either.
"""

import os

from gi.repository import Gio, GLib

from . import files_tint as tint

MODE = "modern"          # "modern" (bundled set) or "system" (icon theme)
COLOR = None             # folder colour for the whole app (#rrggbb) or None = classic

# Customize folder (GIO metadata; metadata::custom-icon is Nautilus' key)
ATTR_CUSTOM_ICON = "metadata::custom-icon"
ATTR_CUSTOM_ICON_NAME = "metadata::custom-icon-name"
ATTR_FOLDER_COLOR = "metadata::harbor-folder-color"
ATTR_EMBLEM = "metadata::harbor-emblem"
METADATA_ATTRS = (ATTR_CUSTOM_ICON, ATTR_CUSTOM_ICON_NAME, ATTR_FOLDER_COLOR, ATTR_EMBLEM)

FOLDER = "harbor-folder"
FILE = "harbor-file"

# One table. First match wins: (icon, mime types/prefixes, extensions).
# A mime entry ending in "/" is a prefix ("image/"); others are matched with
# Gio.content_type_is_a, so subclasses (e.g. text/x-python is a text/plain)
# land on the most specific row listed first.
TYPE_TABLE = (
    ("harbor-file-pdf", ("application/pdf",), (".pdf",)),
    ("harbor-file-diskimage",
     ("application/x-cd-image", "application/x-raw-disk-image", "application/x-iso9660-image",
      "application/x-apple-diskimage", "application/x-qemu-disk", "application/x-virtualbox-vdi",
      "application/x-raw-disk-image-xz-compressed"),
     (".iso", ".img", ".dmg", ".qcow2", ".vdi", ".vmdk", ".vhd", ".vhdx")),
    ("harbor-file-archive",
     ("application/zip", "application/x-tar", "application/gzip", "application/x-compressed-tar",
      "application/x-bzip", "application/x-bzip2", "application/x-xz", "application/x-7z-compressed",
      "application/vnd.rar", "application/x-rar", "application/zstd", "application/x-zstd-compressed-tar",
      "application/x-xz-compressed-tar", "application/x-bzip-compressed-tar",
      "application/x-lzma", "application/x-cpio", "application/vnd.debian.binary-package",
      "application/x-rpm", "application/java-archive"),
     (".zip", ".tar", ".gz", ".tgz", ".bz2", ".xz", ".7z", ".rar", ".zst", ".deb", ".rpm", ".jar")),
    ("harbor-file-spreadsheet",
     ("application/vnd.oasis.opendocument.spreadsheet", "application/vnd.ms-excel",
      "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", "text/csv",
      "text/tab-separated-values"),
     (".ods", ".xls", ".xlsx", ".csv", ".tsv")),
    ("harbor-file-presentation",
     ("application/vnd.oasis.opendocument.presentation", "application/vnd.ms-powerpoint",
      "application/vnd.openxmlformats-officedocument.presentationml.presentation"),
     (".odp", ".ppt", ".pptx", ".key")),
    ("harbor-file-document",
     ("application/vnd.oasis.opendocument.text", "application/msword", "application/rtf",
      "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
      "application/x-abiword", "application/epub+zip"),
     (".odt", ".doc", ".docx", ".rtf", ".epub")),
    ("harbor-file-web", ("text/html", "application/xhtml+xml"), (".html", ".htm", ".xhtml")),
    ("harbor-file-font", ("font/", "application/x-font-ttf", "application/x-font-otf",
                          "application/vnd.ms-opentype", "application/x-font-type1"),
     (".ttf", ".otf", ".woff", ".woff2", ".pfb")),
    ("harbor-file-executable",
     ("application/x-executable", "application/x-pie-executable", "application/x-sharedlib",
      "application/x-shellscript", "application/x-msdownload", "application/x-ms-dos-executable",
      "application/vnd.appimage", "application/x-desktop", "application/x-msi"),
     (".sh", ".run", ".appimage", ".exe", ".msi", ".bin", ".desktop", ".so")),
    ("harbor-file-image", ("image/",), (".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".bmp",
                                        ".tif", ".tiff", ".heic", ".avif", ".jxl", ".ico")),
    ("harbor-file-audio", ("audio/",), (".mp3", ".ogg", ".flac", ".wav", ".m4a", ".opus", ".aac")),
    ("harbor-file-video", ("video/",), (".mp4", ".mkv", ".webm", ".avi", ".mov", ".m4v")),
    ("harbor-file-code",
     ("text/x-python", "text/x-csrc", "text/x-chdr", "text/x-c++src", "text/x-java", "text/x-rust",
      "text/x-go", "text/javascript", "application/javascript", "application/json",
      "application/x-php", "text/x-script.python", "application/xml", "text/xml", "text/css",
      "text/x-makefile", "text/x-cmake", "application/x-yaml", "application/toml",
      "text/x-sql", "application/sql", "application/x-perl", "application/x-ruby",
      "text/x-kotlin", "text/x-csharp", "application/x-m4", "text/x-lua"),
     (".py", ".c", ".h", ".cpp", ".hpp", ".cc", ".rs", ".go", ".js", ".ts", ".tsx", ".jsx",
      ".json", ".java", ".kt", ".php", ".rb", ".pl", ".lua", ".css", ".scss", ".xml", ".yml",
      ".yaml", ".toml", ".ini", ".sql", ".cs", ".swift", ".dart", ".vala", ".mk", ".cmake")),
    ("harbor-file-text", ("text/",), (".txt", ".md", ".log", ".rst", ".nfo")),
)

# Folder emblems for the XDG user folders.
SPECIAL_DIRS = (
    (GLib.UserDirectory.DIRECTORY_DESKTOP, "harbor-folder-desktop"),
    (GLib.UserDirectory.DIRECTORY_DOCUMENTS, "harbor-folder-documents"),
    (GLib.UserDirectory.DIRECTORY_DOWNLOAD, "harbor-folder-downloads"),
    (GLib.UserDirectory.DIRECTORY_MUSIC, "harbor-folder-music"),
    (GLib.UserDirectory.DIRECTORY_PICTURES, "harbor-folder-pictures"),
    (GLib.UserDirectory.DIRECTORY_VIDEOS, "harbor-folder-videos"),
    (GLib.UserDirectory.DIRECTORY_PUBLIC_SHARE, "harbor-folder-public"),
    (GLib.UserDirectory.DIRECTORY_TEMPLATES, "harbor-folder-templates"),
)

# Theme icon names used around the UI -> bundled name (Modern mode).
THEMED = {
    "folder": FOLDER, "folder-symbolic": FOLDER,
    "user-home": "harbor-folder-home", "user-home-symbolic": "harbor-folder-home",
    "go-home": "harbor-folder-home", "go-home-symbolic": "harbor-quick-access",
    "user-desktop": "harbor-folder-desktop", "user-desktop-symbolic": "harbor-folder-desktop",
    "folder-documents": "harbor-folder-documents",
    "folder-documents-symbolic": "harbor-folder-documents",
    "folder-download": "harbor-folder-downloads", "folder-download-symbolic": "harbor-folder-downloads",
    "folder-music": "harbor-folder-music", "folder-music-symbolic": "harbor-folder-music",
    "folder-pictures": "harbor-folder-pictures", "folder-pictures-symbolic": "harbor-folder-pictures",
    "folder-videos": "harbor-folder-videos", "folder-videos-symbolic": "harbor-folder-videos",
    "folder-publicshare": "harbor-folder-public", "folder-publicshare-symbolic": "harbor-folder-public",
    "folder-templates": "harbor-folder-templates",
    "folder-templates-symbolic": "harbor-folder-templates",
    "computer": "harbor-thispc", "computer-symbolic": "harbor-thispc",
    "drive-harddisk": "harbor-drive", "drive-harddisk-symbolic": "harbor-drive",
    "drive-removable-media": "harbor-drive-usb", "drive-removable-media-symbolic": "harbor-drive-usb",
    "media-removable": "harbor-drive-usb", "media-removable-symbolic": "harbor-drive-usb",
    "drive-optical": "harbor-drive-optical", "drive-optical-symbolic": "harbor-drive-optical",
    "media-optical": "harbor-drive-optical", "media-optical-symbolic": "harbor-drive-optical",
    "network-workgroup": "harbor-network", "network-workgroup-symbolic": "harbor-network",
    "network-server": "harbor-network", "folder-remote": "harbor-network",
    "folder-remote-symbolic": "harbor-network",
    "user-trash": "harbor-trash", "user-trash-symbolic": "harbor-trash",
    "user-trash-full": "harbor-trash-full", "user-trash-full-symbolic": "harbor-trash-full",
    "starred": "harbor-quick-access", "starred-symbolic": "harbor-quick-access",
    "text-x-generic": FILE,
}

ALL_ICONS = sorted({FOLDER, FILE, "harbor-folder-home", "harbor-thispc", "harbor-drive",
                    "harbor-drive-usb", "harbor-drive-optical", "harbor-network", "harbor-trash",
                    "harbor-trash-full", "harbor-quick-access"}
                   | {n for _w, n in SPECIAL_DIRS} | {row[0] for row in TYPE_TABLE})


def modern():
    return MODE == "modern"


def _is_a(ct, want):
    if want.endswith("/"):
        return ct.startswith(want)
    if ct == want:
        return True
    try:
        return Gio.content_type_is_a(ct, want)
    except TypeError:
        return False


def icon_for_type(content_type, name=""):
    """Bundled file icon name for a content type (falls back to the extension)."""
    ct = (content_type or "").lower()
    if ct and ct not in ("application/octet-stream", "inode/x-unknown"):
        for icon, mimes, _exts in TYPE_TABLE:
            if any(_is_a(ct, m) for m in mimes):
                return icon
    low = (name or "").lower()
    if low.endswith((".tar.gz", ".tar.xz", ".tar.bz2", ".tar.zst")):
        return "harbor-file-archive"
    ext = os.path.splitext(low)[1]
    if ext:
        for icon, _mimes, exts in TYPE_TABLE:
            if ext in exts:
                return icon
    return FILE


_special = None


def special_dirs():
    """{normalised path: folder icon} for the XDG user folders + home."""
    global _special
    if _special is None:
        home = os.path.normpath(os.path.expanduser("~"))
        out = {home: "harbor-folder-home"}
        from . import files_core as fc
        by_dir = {which: icon for which, icon in SPECIAL_DIRS}
        kind_dir = {k: d for k, d, _i in fc.USER_FOLDERS + fc.EXTRA_USER_FOLDERS}
        for kind, p, _icon in fc.user_folders(home, extra=True):
            icon = by_dir.get(kind_dir[kind])
            if icon:
                out[os.path.normpath(p)] = icon
        _special = out
    return _special


def icon_for_folder(path=None):
    if path:
        return special_dirs().get(os.path.normpath(path), FOLDER)
    return FOLDER


def icon_name_for(content_type, name="", is_dir=False, path=None):
    return icon_for_folder(path) if is_dir else icon_for_type(content_type, name)


def tinted(name, color=None, emblem=None):
    """A Harbor folder icon name in the folder colour (per-folder > app-wide)."""
    if not modern() or not tint.is_folder_icon(name):
        return name
    c = color if tint.valid_hex(color) else COLOR
    return tint.icon_name(name, c, emblem if emblem in tint.EMBLEMS else None)


def custom_of(info):
    """(custom icon uri, custom icon name, colour, emblem) from GIO metadata."""
    def get(attr):
        try:
            v = info.get_attribute_string(attr)
        except TypeError:
            v = None
        return v or None
    return tuple(get(a) for a in METADATA_ATTRS)


def custom_gicon(info, is_dir=False):
    """Icon chosen with Customize folder, or None."""
    uri, name, color, emblem = custom_of(info)
    if uri:
        f = Gio.File.new_for_uri(uri) if "://" in uri else Gio.File.new_for_path(uri)
        return Gio.FileIcon.new(f)
    if name:
        return Gio.ThemedIcon.new(tinted(name, color if is_dir else None, emblem))
    return None


def item_gicon(info, path=None, is_dir=False):
    """Icon for a list item: the bundled one in Modern mode, else the theme's."""
    own = custom_gicon(info, is_dir)
    if own is not None:
        return own
    if not modern():
        return info.get_icon()
    ct = info.get_content_type() or info.get_attribute_string("standard::fast-content-type") or ""
    if is_dir and ct not in ("inode/directory", ""):
        # mountables and the like keep their own icon (drives in This PC etc.)
        own = info.get_icon()
        return own or Gio.ThemedIcon.new(FOLDER)
    name = icon_name_for(ct, info.get_display_name() or "", is_dir, path)
    if is_dir:
        _u, _n, color, emblem = custom_of(info)
        name = tinted(name, color, emblem)
    return Gio.ThemedIcon.new(name)


def themed(name):
    """Map a theme icon name used by the UI to the bundled one (Modern mode)."""
    if not modern() or not isinstance(name, str):
        return name
    return tinted(THEMED.get(name, name))


def themed_gicon(icon):
    """Same as themed() for a Gio.Icon (drive / mount icons)."""
    if not modern() or icon is None:
        return icon
    if isinstance(icon, Gio.ThemedIcon):
        for n in icon.get_names():
            if n in THEMED:
                return Gio.ThemedIcon.new(tinted(THEMED[n]))
            for key, val in (("optical", "harbor-drive-optical"), ("removable", "harbor-drive-usb"),
                             ("usb", "harbor-drive-usb"), ("harddisk", "harbor-drive"),
                             ("network", "harbor-network"), ("remote", "harbor-network")):
                if key in n:
                    return Gio.ThemedIcon.new(val)
    return icon
