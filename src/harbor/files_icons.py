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
GLOSSY = False           # glossy twins (<name>-glossy) of folder/file icons: Lumina Glass
                         # look with Modern icons, or Icons > Glossy

# Customize folder (GIO metadata; metadata::custom-icon is Nautilus' key)
ATTR_CUSTOM_ICON = "metadata::custom-icon"
ATTR_CUSTOM_ICON_NAME = "metadata::custom-icon-name"
ATTR_FOLDER_COLOR = "metadata::harbor-folder-color"
ATTR_EMBLEM = "metadata::harbor-emblem"
# the Harbor glyph behind an exported global icon, so Customize can reopen it
ATTR_GLYPH = "metadata::harbor-icon-name"
METADATA_ATTRS = (ATTR_CUSTOM_ICON, ATTR_CUSTOM_ICON_NAME, ATTR_FOLDER_COLOR, ATTR_EMBLEM,
                  ATTR_GLYPH)

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

# Per-extension icons: one harbor-ext-<key> SVG each (tools/make_icons.py
# draws them: the category glyph, then a coloured badge with the label).
# The extension wins over the content type, so every .deb, .py or .mp4 looks
# the same everywhere in the app. Rows: (key, category, badge colour, exts[, label]).
EXT_TABLE = (
    # archives and packages
    ("zip", "archive", "#d68910", (".zip",)),
    ("rar", "archive", "#8e44ad", (".rar",)),
    ("7z", "archive", "#2e7d32", (".7z",)),
    ("tar", "archive", "#6d4c41", (".tar",)),
    ("tgz", "archive", "#8d6e63", (".tar.gz", ".tgz")),
    ("gz", "archive", "#a1887f", (".gz",)),
    ("txz", "archive", "#5d4037", (".tar.xz", ".txz")),
    ("xz", "archive", "#4e342e", (".xz",)),
    ("tbz", "archive", "#795548", (".tar.bz2", ".tbz2", ".tbz")),
    ("bz2", "archive", "#6d5a4f", (".bz2",)),
    ("tzst", "archive", "#455a64", (".tar.zst",)),
    ("zst", "archive", "#37474f", (".zst",)),
    ("deb", "archive", "#a80030", (".deb",)),
    ("rpm", "archive", "#cc0000", (".rpm",)),
    ("apk", "archive", "#3ddc84", (".apk", ".apks", ".xapk")),
    ("aab", "archive", "#2e9d5b", (".aab",)),
    ("jar", "archive", "#e76f00", (".jar", ".war")),
    ("snap", "archive", "#e95420", (".snap",)),
    ("flatpak", "archive", "#4a86cf", (".flatpak", ".flatpakref"), "FPAK"),
    # programs
    ("exe", "executable", "#0078d4", (".exe",)),
    ("msi", "executable", "#00a4ef", (".msi",)),
    ("appimage", "executable", "#4a90d9", (".appimage",), "AIMG"),
    ("sh", "executable", "#2f3b40", (".sh", ".bash", ".zsh")),
    ("run", "executable", "#546e7a", (".run",)),
    ("bin", "executable", "#607d8b", (".bin",)),
    ("desktop", "executable", "#5c6bc0", (".desktop",), "DESK"),
    ("so", "executable", "#78909c", (".so",)),
    ("bat", "executable", "#455a64", (".bat", ".cmd")),
    ("ps1", "executable", "#012456", (".ps1",)),
    # disk images
    ("iso", "diskimage", "#546e7a", (".iso",)),
    ("img", "diskimage", "#607d8b", (".img",)),
    ("dmg", "diskimage", "#8e8e93", (".dmg",)),
    ("qcow2", "diskimage", "#ef6c00", (".qcow2", ".qcow"), "QCOW"),
    ("vdi", "diskimage", "#1565c0", (".vdi",)),
    ("vmdk", "diskimage", "#607d8b", (".vmdk",)),
    ("vhd", "diskimage", "#0078d4", (".vhd", ".vhdx")),
    # documents
    ("pdf", "pdf", "#d93838", (".pdf",)),
    ("docx", "document", "#2b579a", (".docx",)),
    ("doc", "document", "#3b6cb5", (".doc",)),
    ("odt", "document", "#0e7ac4", (".odt",)),
    ("rtf", "document", "#5c6bc0", (".rtf",)),
    ("epub", "document", "#7cb342", (".epub",)),
    ("xlsx", "spreadsheet", "#1e7145", (".xlsx",)),
    ("xls", "spreadsheet", "#2e8b57", (".xls",)),
    ("ods", "spreadsheet", "#43a047", (".ods",)),
    ("csv", "spreadsheet", "#16a085", (".csv",)),
    ("tsv", "spreadsheet", "#00897b", (".tsv",)),
    ("pptx", "presentation", "#c43e1c", (".pptx",)),
    ("ppt", "presentation", "#d35230", (".ppt",)),
    ("odp", "presentation", "#e8590c", (".odp",)),
    ("key", "presentation", "#1e88e5", (".key",)),
    ("txt", "text", "#78909c", (".txt",)),
    ("md", "text", "#37474f", (".md", ".markdown")),
    ("log", "text", "#8d6e63", (".log",)),
    ("rst", "text", "#546e7a", (".rst",)),
    ("nfo", "text", "#6d8086", (".nfo",)),
    ("srt", "text", "#f9a825", (".srt",)),
    ("ass", "text", "#fbc02d", (".ass", ".ssa")),
    ("vtt", "text", "#ffb300", (".vtt",)),
    # web and fonts
    ("html", "web", "#e44d26", (".html", ".htm", ".xhtml")),
    ("ttf", "font", "#3f51b5", (".ttf",)),
    ("otf", "font", "#5e35b1", (".otf",)),
    ("woff", "font", "#3949ab", (".woff", ".woff2")),
    # images
    ("png", "image", "#1e88e5", (".png",)),
    ("jpg", "image", "#00897b", (".jpg", ".jpeg", ".jpe")),
    ("gif", "image", "#8e24aa", (".gif",)),
    ("webp", "image", "#43a047", (".webp",)),
    ("svg", "image", "#ff9800", (".svg", ".svgz")),
    ("bmp", "image", "#5d4037", (".bmp",)),
    ("tif", "image", "#6d4c41", (".tif", ".tiff")),
    ("heic", "image", "#ef6c00", (".heic", ".heif")),
    ("avif", "image", "#c2185b", (".avif",)),
    ("jxl", "image", "#3949ab", (".jxl",)),
    ("ico", "image", "#0097a7", (".ico",)),
    ("psd", "image", "#1565c0", (".psd",)),
    ("xcf", "image", "#6d6150", (".xcf",)),
    ("raw", "image", "#424242", (".raw", ".dng", ".cr2", ".cr3", ".nef", ".arw", ".orf", ".rw2")),
    # audio
    ("mp3", "audio", "#e8590c", (".mp3",)),
    ("flac", "audio", "#f57c00", (".flac",)),
    ("wav", "audio", "#0288d1", (".wav",)),
    ("ogg", "audio", "#7b1fa2", (".ogg", ".oga")),
    ("opus", "audio", "#00796b", (".opus",)),
    ("m4a", "audio", "#c2185b", (".m4a",)),
    ("aac", "audio", "#5d4037", (".aac",)),
    ("wma", "audio", "#1565c0", (".wma",)),
    ("mid", "audio", "#6a1b9a", (".mid", ".midi")),
    ("amr", "audio", "#455a64", (".amr",)),
    # video
    ("mp4", "video", "#5e35b1", (".mp4",)),
    ("mkv", "video", "#2e7d32", (".mkv",)),
    ("webm", "video", "#00838f", (".webm",)),
    ("avi", "video", "#455a64", (".avi",)),
    ("mov", "video", "#1565c0", (".mov",)),
    ("m4v", "video", "#7e57c2", (".m4v",)),
    ("wmv", "video", "#0277bd", (".wmv",)),
    ("flv", "video", "#c62828", (".flv",)),
    ("3gp", "video", "#558b2f", (".3gp",)),
    ("mpg", "video", "#4527a0", (".mpg", ".mpeg")),
    # code and config
    ("py", "code", "#3776ab", (".py", ".pyw", ".pyi")),
    ("ipynb", "code", "#f37626", (".ipynb",), "NB"),
    ("c", "code", "#283593", (".c",)),
    ("h", "code", "#5c6bc0", (".h", ".hpp", ".hh")),
    ("cpp", "code", "#00599c", (".cpp", ".cc", ".cxx")),
    ("rs", "code", "#b7410e", (".rs",)),
    ("go", "code", "#00add8", (".go",)),
    ("js", "code", "#f7df1e", (".js", ".mjs", ".cjs")),
    ("ts", "code", "#3178c6", (".ts", ".mts", ".cts")),
    ("tsx", "code", "#0288d1", (".tsx",)),
    ("jsx", "code", "#00acc1", (".jsx",)),
    ("vue", "code", "#41b883", (".vue",)),
    ("json", "code", "#616161", (".json", ".jsonc", ".json5")),
    ("java", "code", "#b07219", (".java",)),
    ("kt", "code", "#7f52ff", (".kt", ".kts")),
    ("php", "code", "#777bb4", (".php",)),
    ("rb", "code", "#cc342d", (".rb",)),
    ("pl", "code", "#39457e", (".pl", ".pm")),
    ("lua", "code", "#000080", (".lua",)),
    ("css", "code", "#264de4", (".css",)),
    ("scss", "code", "#cd6799", (".scss", ".sass")),
    ("xml", "code", "#0060ac", (".xml",)),
    ("yml", "code", "#cb171e", (".yml", ".yaml")),
    ("toml", "code", "#9c4221", (".toml",)),
    ("ini", "code", "#6d8086", (".ini", ".conf", ".cfg")),
    ("sql", "code", "#e38c00", (".sql", ".sqlite", ".db")),
    ("cs", "code", "#68217a", (".cs",)),
    ("swift", "code", "#f05138", (".swift",)),
    ("dart", "code", "#0175c2", (".dart",)),
    ("vala", "code", "#7239b3", (".vala",)),
    ("mk", "code", "#427819", (".mk",)),
    ("cmake", "code", "#064f8c", (".cmake",), "CMK"),
    ("gradle", "code", "#02303a", (".gradle",), "GRDL"),
    ("tex", "code", "#008080", (".tex", ".bib")),
    ("r", "code", "#276dc3", (".r", ".rmd")),
    ("m", "code", "#e16737", (".m",)),
)

# Files known by their whole name rather than an extension.
NAME_ICONS = {"makefile": "harbor-ext-mk", "gnumakefile": "harbor-ext-mk",
              "cmakelists.txt": "harbor-ext-cmake", "build.gradle": "harbor-ext-gradle"}


def ext_label(row):
    return (row[4] if len(row) > 4 else row[0]).upper()


EXT_ICONS = {e: "harbor-ext-" + row[0] for row in EXT_TABLE for e in row[3]}
# multi-part extensions (".tar.gz") are checked first, longest first
_COMPOUND = sorted((e for e in EXT_ICONS if e.count(".") > 1), key=len, reverse=True)


def ext_icon(name):
    """harbor-ext-* icon for a file name, or None."""
    low = os.path.basename(name or "").lower()
    if not low:
        return None
    if low in NAME_ICONS:
        return NAME_ICONS[low]
    for e in _COMPOUND:
        if low.endswith(e) and len(low) > len(e):
            return EXT_ICONS[e]
    ext = os.path.splitext(low)[1]
    return EXT_ICONS.get(ext) if ext else None


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
                   | {n for _w, n in SPECIAL_DIRS} | {row[0] for row in TYPE_TABLE}
                   | set(EXT_ICONS.values()) | set(NAME_ICONS.values()))


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
    """Bundled file icon: the extension's own icon, else the content type's category."""
    own = ext_icon(name)
    if own:
        return own
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


def gloss(name):
    """The glossy twin of a folder / file icon name while GLOSSY is on."""
    if (GLOSSY and modern() and isinstance(name, str) and not name.endswith("-glossy")
            and name.startswith(("harbor-folder", "harbor-file", "harbor-ext-"))):
        return name + "-glossy"
    return name


def tinted(name, color=None, emblem=None):
    """A Harbor folder icon name in the folder colour (per-folder > app-wide)."""
    name = gloss(name)
    if not modern() or not tint.is_folder_icon(name):
        return name
    c = color if tint.valid_hex(color) else COLOR
    return tint.icon_name(name, c, emblem if emblem in tint.EMBLEMS else None)


def global_of(info):
    """(icon uri, icon name, colour, emblem) from GIO metadata, checked.

    Any app can write these keys, so an unsafe picture reference (remote,
    not a regular file, too big) or a malformed name is dropped.
    """
    from . import files_iconstore as store

    def get(attr):
        try:
            v = info.get_attribute_string(attr) if info is not None else None
        except TypeError:
            v = None
        return v or None
    st = store.clean_state({"icon_uri": get(ATTR_CUSTOM_ICON), "icon_name": get(ATTR_CUSTOM_ICON_NAME),
                            "color": get(ATTR_FOLDER_COLOR), "emblem": get(ATTR_EMBLEM)}) or {}
    return tuple(st.get(k) for k in store.FIELDS)


def custom_of(info, path=None):
    """(icon uri, icon name, colour, emblem) chosen with Customize folder.

    A Harbor-only choice for this folder wins; else the global metadata.
    """
    from . import files_iconstore as store
    if path:
        own = store.store().get(path)
        if own:
            return tuple(own.get(k) for k in store.FIELDS)
    return global_of(info)


def custom_gicon(info, is_dir=False, path=None):
    """Icon chosen with Customize folder, or None."""
    uri, name, color, emblem = custom_of(info, path)
    if uri:
        from . import files_iconstore as store
        p = store.safe_icon_path(uri)
        return Gio.FileIcon.new(Gio.File.new_for_path(p)) if p else None
    if name:
        return Gio.ThemedIcon.new(tinted(name, color if is_dir else None, emblem))
    return None


def item_gicon(info, path=None, is_dir=False):
    """Icon for a list item: the bundled one in Modern mode, else the theme's."""
    own = custom_gicon(info, is_dir, path if is_dir else None)
    if own is not None:
        return own
    if not modern():
        return info.get_icon()
    ct = info.get_content_type() or info.get_attribute_string("standard::fast-content-type") or ""
    if is_dir and ct not in ("inode/directory", ""):
        # mountables and the like keep their own icon (drives in This PC etc.)
        own = info.get_icon()
        return own or Gio.ThemedIcon.new(tinted(FOLDER))
    name = icon_name_for(ct, info.get_display_name() or "", is_dir, path)
    if is_dir:
        _u, _n, color, emblem = custom_of(info, path)
        name = tinted(name, color, emblem)
    else:
        name = gloss(name)
    return Gio.ThemedIcon.new(name)


def place_icon(path, default):
    """Icon for a folder shown by path alone (Quick access tiles, Home page,
    sidebar): the Customize folder choice when there is one, else default."""
    if not path or not os.path.isabs(path):
        return default
    info = None
    try:
        info = Gio.File.new_for_path(path).query_info(
            ",".join(METADATA_ATTRS), Gio.FileQueryInfoFlags.NONE, None)
    except GLib.Error:
        pass
    uri, name, color, emblem = custom_of(info, path)
    if not (uri or name or color or emblem):
        return default
    own = custom_gicon(info, True, path)
    if own is not None:
        return own
    if not modern():
        return default
    return Gio.ThemedIcon.new(tinted(icon_for_folder(path), color, emblem))


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
