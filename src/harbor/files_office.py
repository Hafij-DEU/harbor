"""Harbor Files: office document previews through Craft Suite (no widgets).

Word (docx/doc/odt/rtf) and Slide (pptx) documents are rendered to PNG by
wordcraft-cli / deckcraft-cli; spreadsheets (xlsx/csv/tsv) are read as CSV
from gridcraft-cli. Everything here runs in a worker thread. Results are
cached in $XDG_CACHE_HOME/harbor/previews/<sha1>.png, keyed by path, mtime,
size, page and scale.

When the CLI is missing or fails, the fallback chain is:
  1. a thumbnail stored inside the zip (Thumbnails/thumbnail.png,
     docProps/thumbnail.jpeg, ...)
  2. the GNOME thumbnail cache (~/.cache/thumbnails/<size>/<md5 uri>.png)
  3. plain text pulled from the document's XML
  4. nothing ("No preview available")
"""

import csv
import hashlib
import io
import json
import os
import re
import shutil
import subprocess
import zipfile

from gi.repository import GLib

from . import files_core as fc

TIMEOUT = 15
CLIS = {"word": "wordcraft-cli", "slides": "deckcraft-cli", "sheet": "gridcraft-cli"}
APP_NAMES = {"word": "Word", "slides": "Slide", "sheet": "Spreadsheet"}
# craft-suite.desktop takes files (%F) and opens each in Word / Slide /
# Spreadsheet by type; the per-app launchers only start a new document.
DESKTOP_IDS = {"word": ("craft-suite.desktop",), "slides": ("craft-suite.desktop",),
               "sheet": ("craft-suite.desktop",)}
SHEET_ROWS, SHEET_COLS = 40, 8
ZIP_THUMBS = ("Thumbnails/thumbnail.png", "docProps/thumbnail.jpeg", "docProps/thumbnail.png",
              "docProps/thumbnail.jpg", "_rels/thumbnail.png")
TEXT_PARTS = {"word": ("word/document.xml", "content.xml"),
              "slides": ("ppt/slides/slide1.xml",),
              "sheet": ("xl/sharedStrings.xml",)}


def tools(which=shutil.which):
    """{"word": path or None, "slides": ..., "sheet": ...}"""
    return {k: which(v) for k, v in CLIS.items()}


def cache_dir():
    return os.path.join(GLib.get_user_cache_dir(), "harbor", "previews")


def cache_key(path, mtime, size, extra=""):
    """sha1 of path + mtime + size (+ page/scale), hex."""
    raw = "%s\0%s\0%s\0%s" % (os.path.abspath(path), int(mtime), int(size), extra)
    return hashlib.sha1(raw.encode("utf-8", "surrogateescape")).hexdigest()


def cache_file(path, mtime, size, extra="", suffix=".png", root=None):
    return os.path.join(root or cache_dir(), cache_key(path, mtime, size, extra) + suffix)


def cache_size(root=None):
    root = root or cache_dir()
    total = 0
    try:
        with os.scandir(root) as it:
            for e in it:
                try:
                    total += e.stat(follow_symlinks=False).st_size
                except OSError:
                    pass
    except OSError:
        pass
    return total


def clear_cache(root=None):
    root = root or cache_dir()
    n = 0
    try:
        with os.scandir(root) as it:
            for e in it:
                if e.is_file(follow_symlinks=False):
                    try:
                        os.unlink(e.path)
                        n += 1
                    except OSError:
                        pass
    except OSError:
        pass
    return n


def _arg(p):
    """These CLIs have no "--": a path that looks like an option is made absolute."""
    return os.path.abspath(p) if p.startswith("-") else p


# ------------------------------------------------------------------ argv builders

def word_render_argv(cli, path, out, page=1, scale=0.6):
    return [cli, "render", _arg(path), _arg(out), "--page", str(int(page)),
            "--scale", "%g" % scale]


def word_info_argv(cli, path):
    return [cli, "info", _arg(path)]


def deck_render_argv(cli, path, out, slide=0, scale=0.5):
    return [cli, "render", _arg(path), "--slide", str(int(slide)), "--scale", "%g" % scale,
            _arg(out)]


def deck_info_argv(cli, path):
    return [cli, "info", _arg(path)]


def sheet_cat_argv(cli, path, rows=SHEET_ROWS, cols=SHEET_COLS, sheet=None):
    argv = [cli, "cat", _arg(path), "--csv", "--range", "A1:%s%d" % (col_name(cols - 1), rows)]
    if sheet:
        argv += ["--sheet", sheet]
    return argv


def sheet_info_argv(cli, path):
    return [cli, "info", _arg(path), "--json"]


def col_name(i):
    s = ""
    i += 1
    while i:
        i, r = divmod(i - 1, 26)
        s = chr(65 + r) + s
    return s


# ------------------------------------------------------------------ running

def _run(argv, timeout=TIMEOUT):
    p = subprocess.run(argv, stdin=subprocess.DEVNULL, capture_output=True, timeout=timeout,
                       env=dict(os.environ, NO_COLOR="1"))
    return p.returncode, p.stdout.decode("utf-8", "replace"), p.stderr.decode("utf-8", "replace")


def _json(text):
    try:
        i = text.index("{")
        return json.loads(text[i:])
    except ValueError:
        return {}


def page_count(kind, cli, path, run=_run):
    """Pages (Word) or slides (Slide); 0 when unknown."""
    try:
        if kind == "word":
            rc, out, _e = run(word_info_argv(cli, path))
            return int(_json(out).get("pages") or 0) if rc == 0 else 0
        if kind == "slides":
            rc, out, _e = run(deck_info_argv(cli, path))
            return len(_json(out).get("slides") or []) if rc == 0 else 0
    except (OSError, subprocess.TimeoutExpired, TypeError, ValueError):
        return 0
    return 0


def sheet_names(cli, path, run=_run):
    try:
        rc, out, _e = run(sheet_info_argv(cli, path))
    except (OSError, subprocess.TimeoutExpired):
        return []
    if rc != 0:
        return []
    return [s.get("name") for s in _json(out).get("sheets") or []
            if isinstance(s, dict) and s.get("name")]


def parse_csv(text, rows=SHEET_ROWS, cols=SHEET_COLS, delimiter=","):
    out = []
    for row in csv.reader(io.StringIO(text), delimiter=delimiter):
        out.append([c for c in row[:cols]] + [""] * max(0, cols - len(row)))
        if len(out) >= rows:
            break
    # drop empty trailing columns
    while out and all(not r[-1] for r in out) and len(out[0]) > 1:
        out = [r[:-1] for r in out]
    return out


def render(kind, path, mtime, size, page=0, scale=None, which=shutil.which, run=_run,
           root=None):
    """Worker: the preview for one document.

    Returns a dict: {"kind": "image", "png": path, "pages": n, "page": i,
    "source": "cli" | "zip" | "gnome"}, {"kind": "table", "rows": [[...]],
    "sheets": [...], "sheet": name}, {"kind": "text", "text": ...} or
    {"kind": "none", "reason": ...}.
    """
    cli = which(CLIS[kind]) if kind in CLIS else None
    root = root or cache_dir()
    errors = []
    if cli:
        try:
            if kind == "sheet":
                sheets = sheet_names(cli, path, run) if not path.lower().endswith(
                    (".csv", ".tsv")) else []
                sheet = sheets[page] if 0 <= page < len(sheets) else None
                rc, out, err = run(sheet_cat_argv(cli, path, sheet=sheet))
                if rc == 0:
                    return {"kind": "table", "rows": parse_csv(out), "sheets": sheets,
                            "sheet": sheet or (sheets[0] if sheets else None),
                            "page": page, "source": "cli"}
                errors.append(err.strip())
            else:
                scale = scale or (0.6 if kind == "word" else 0.5)
                extra = "%s:%d:%g" % (kind, page, scale)
                out_png = cache_file(path, mtime, size, extra, root=root)
                pages = page_count(kind, cli, path, run)
                if not os.path.exists(out_png):
                    os.makedirs(root, exist_ok=True)
                    tmp = out_png + ".tmp.png"
                    argv = word_render_argv(cli, path, tmp, page + 1, scale) if kind == "word" \
                        else deck_render_argv(cli, path, tmp, page, scale)
                    rc, _o, err = run(argv)
                    if rc == 0 and os.path.exists(tmp) and os.path.getsize(tmp) > 0:
                        os.replace(tmp, out_png)
                    else:
                        try:
                            os.unlink(tmp)
                        except OSError:
                            pass
                        errors.append(err.strip())
                if os.path.exists(out_png):
                    return {"kind": "image", "png": out_png, "pages": pages, "page": page,
                            "source": "cli"}
        except subprocess.TimeoutExpired:
            errors.append("timed out")
        except OSError as e:
            errors.append(str(e))
    return fallback(kind, path, mtime, size, root=root,
                    reason="; ".join(e for e in errors if e) or
                    ("%s is not installed" % CLIS.get(kind, "?") if not cli else ""))


def fallback(kind, path, mtime, size, root=None, reason="", thumb_root=None):
    """The chain used without Craft Suite (see the module docstring)."""
    root = root or cache_dir()
    # csv / tsv: read them ourselves
    low = path.lower()
    if kind == "sheet" and low.endswith((".csv", ".tsv")):
        try:
            text, _cut = fc.read_text_preview(path)
            return {"kind": "table", "rows": parse_csv(text, delimiter="\t" if low.endswith(
                ".tsv") else ","), "sheets": [], "sheet": None, "page": 0, "source": "text"}
        except OSError:
            pass
    t = zip_thumbnail(path, mtime, size, root)
    if t:
        return {"kind": "image", "png": t, "pages": 0, "page": 0, "source": "zip"}
    t = gnome_thumbnail(path, thumb_root)
    if t:
        return {"kind": "image", "png": t, "pages": 0, "page": 0, "source": "gnome"}
    text = zip_text(kind, path)
    if text:
        return {"kind": "text", "text": text, "source": "xml"}
    return {"kind": "none", "reason": reason}


def thumbnail(path, mtime, size, which=shutil.which, run=_run, root=None):
    """Worker: a small picture of a document for Large icons, or None.

    The zip's own thumbnail and GNOME's cache are cheap; the Craft Suite CLI
    renders page 1 only when neither exists.
    """
    kind = fc.office_kind(path)
    if not kind:
        return None
    t = zip_thumbnail(path, mtime, size, root) or gnome_thumbnail(path)
    if t:
        return t
    if kind == "sheet":
        return None
    cli = which(CLIS[kind])
    if not cli:
        return None
    root = root or cache_dir()
    out = cache_file(path, mtime, size, "thumb:%s" % kind, root=root)
    if os.path.exists(out):
        return out
    os.makedirs(root, exist_ok=True)
    tmp = out + ".tmp.png"
    argv = word_render_argv(cli, path, tmp, 1, 0.25) if kind == "word" \
        else deck_render_argv(cli, path, tmp, 0, 0.2)
    try:
        rc, _o, _e = run(argv)
    except (OSError, subprocess.TimeoutExpired):
        rc = 1
    if rc == 0 and os.path.exists(tmp) and os.path.getsize(tmp) > 0:
        os.replace(tmp, out)
        return out
    try:
        os.unlink(tmp)
    except OSError:
        pass
    return None


def zip_thumbnail(path, mtime, size, root=None):
    try:
        with zipfile.ZipFile(path) as zf:
            names = set(zf.namelist())
            for n in ZIP_THUMBS:
                if n in names:
                    info = zf.getinfo(n)
                    if info.file_size > 8 * 1024 * 1024:
                        continue
                    ext = os.path.splitext(n)[1].lower() or ".png"
                    out = cache_file(path, mtime, size, "zipthumb", suffix=ext, root=root)
                    if not os.path.exists(out):
                        os.makedirs(os.path.dirname(out), exist_ok=True)
                        with zf.open(info) as src, open(out + ".tmp", "wb") as dst:
                            dst.write(src.read())
                        os.replace(out + ".tmp", out)
                    return out
    except (OSError, zipfile.BadZipFile, KeyError, RuntimeError):
        return None
    return None


def gnome_thumbnail(path, thumb_root=None):
    from gi.repository import Gio
    uri = Gio.File.new_for_path(os.path.abspath(path)).get_uri()
    md5 = hashlib.md5(uri.encode()).hexdigest()
    base = thumb_root or os.path.join(GLib.get_user_cache_dir(), "thumbnails")
    for size in ("x-large", "large", "normal"):
        p = os.path.join(base, size, md5 + ".png")
        if os.path.isfile(p):
            return p
    return None


_TAG = re.compile(r"<[^>]+>")
_PARA = re.compile(r"</(w:p|a:p|text:p|text:h|si)>")


def zip_text(kind, path, limit=4000):
    try:
        with zipfile.ZipFile(path) as zf:
            names = set(zf.namelist())
            for part in TEXT_PARTS.get(kind, ()):
                if part in names:
                    info = zf.getinfo(part)
                    if info.file_size > 32 * 1024 * 1024:
                        return ""
                    xml = zf.read(part).decode("utf-8", "replace")
                    xml = _PARA.sub("\n", xml)
                    text = _TAG.sub("", xml)
                    import html
                    text = html.unescape(text)
                    text = "\n".join(line.strip() for line in text.splitlines() if line.strip())
                    return text[:limit]
    except (OSError, zipfile.BadZipFile, KeyError, RuntimeError):
        return ""
    return ""


def app_for(kind):
    """Gio.DesktopAppInfo of the Craft Suite app for a kind, or None."""
    from gi.repository import Gio
    for did in DESKTOP_IDS.get(kind, ()):
        try:
            a = Gio.DesktopAppInfo.new(did)
        except TypeError:
            a = None
        if a is not None:
            return a
    want = APP_NAMES.get(kind, "").lower()
    for a in Gio.AppInfo.get_all():
        name = (a.get_name() or "").lower()
        exe = (a.get_executable() or "").lower()
        if want and (name == want or exe.startswith(CLIS[kind].split("-")[0])):
            return a
    return None
