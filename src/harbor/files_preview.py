"""Harbor Files: the preview pane (Alt+P).

Shows the selected file: images, the start of text files, the first page of
a PDF, video/audio players (never autoplay), a font sample, a folder's item
count, or a big icon with "No preview available". Every load runs off the
main loop; a generation counter drops results that arrive after the
selection moved on.
"""

import os
import shutil
import subprocess
import tempfile
import threading

from gi.repository import Gdk, GdkPixbuf, Gio, GLib, Gtk, Pango

from . import files_core as fc
from . import files_icons

PREVIEW_MAX_IMAGE_BYTES = 64 * 1024 * 1024
PREVIEW_PX = 640
PDF_TIMEOUT = 15
FONT_SAMPLE = "The quick brown fox jumps over the lazy dog\n0123456789 !?&@"


def _image_texture(path, px=PREVIEW_PX):
    """Worker: a scaled pixbuf for an image (None when it can't be read)."""
    try:
        if os.path.getsize(path) > PREVIEW_MAX_IMAGE_BYTES:
            return None
        return GdkPixbuf.Pixbuf.new_from_file_at_scale(path, px, px, True)
    except (GLib.Error, OSError):
        return None


def pdf_argv(path, prefix, px=PREVIEW_PX):
    """pdftoppm command for the first page as one PNG (prefix + ".png")."""
    return ["pdftoppm", "-f", "1", "-l", "1", "-png", "-singlefile",
            "-scale-to", str(px), "--", path, prefix]


def _pdf_page(path, px=PREVIEW_PX):
    """Worker: first PDF page as a pixbuf via pdftoppm (None on failure)."""
    if not shutil.which("pdftoppm"):
        return None
    tmp = tempfile.mkdtemp(prefix="harbor-preview-")
    try:
        prefix = os.path.join(tmp, "page")
        subprocess.run(pdf_argv(path, prefix, px), stdout=subprocess.DEVNULL,
                       stderr=subprocess.DEVNULL, timeout=PDF_TIMEOUT, check=True)
        return GdkPixbuf.Pixbuf.new_from_file(prefix + ".png")
    except (OSError, subprocess.SubprocessError, GLib.Error):
        return None
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def _font_family(path):
    """Worker: the family name of a font file (fc-scan), or None."""
    if not shutil.which("fc-scan"):
        return None
    try:
        out = subprocess.run(["fc-scan", "--format", "%{family[0]}\n", "--", path],
                             capture_output=True, text=True, timeout=5).stdout
    except (OSError, subprocess.SubprocessError):
        return None
    first = out.strip().splitlines()[0] if out.strip() else ""
    return first or None


def _count(path):
    with os.scandir(path) as it:
        return sum(1 for _ in it)


class PreviewPane(Gtk.Box):
    def __init__(self, win):
        super().__init__(orientation=Gtk.Orientation.VERTICAL, width_request=320,
                         css_classes=["hbf-pane", "hbf-preview"])
        self.win = win
        self._gen = 0
        self._key = None
        self._fonts_added = set()
        self.content = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, vexpand=True,
                               margin_start=14, margin_end=14, margin_top=14, margin_bottom=14,
                               spacing=8)
        # A non-scrolling ScrolledWindow clips and, unlike a Box, does not
        # report the picture's natural width, so the pane stays 320 px wide
        # instead of squeezing the file list.
        self.append(Gtk.ScrolledWindow(child=self.content, vexpand=True,
                                       hscrollbar_policy=Gtk.PolicyType.NEVER,
                                       vscrollbar_policy=Gtk.PolicyType.NEVER,
                                       propagate_natural_width=False))

    # ------------------------------------------------------------ helpers
    def _clear(self):
        self._gen += 1
        while (c := self.content.get_first_child()) is not None:
            if isinstance(c, Gtk.Video) and c.get_media_stream() is not None:
                c.get_media_stream().pause()
            self.content.remove(c)
        return self._gen

    def _message(self, icon, text, sub=None):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10, vexpand=True,
                      valign=Gtk.Align.CENTER)
        img = Gtk.Image(pixel_size=96)
        if isinstance(icon, Gio.Icon):
            img.set_from_gicon(files_icons.themed_gicon(icon))
        else:
            img.set_from_icon_name(files_icons.themed(icon or "text-x-generic"))
        box.append(img)
        lab = Gtk.Label(label=text, wrap=True, justify=Gtk.Justification.CENTER,
                        css_classes=["hbf-pane-title"])
        lab.set_wrap_mode(Pango.WrapMode.WORD_CHAR)
        box.append(lab)
        if sub:
            s = Gtk.Label(label=sub, wrap=True, justify=Gtk.Justification.CENTER,
                          css_classes=["hb-dim"])
            box.append(s)
        self.content.append(box)
        return box

    def _later(self, gen, work, done):
        """Run work() in a thread; done(result) on the main loop if still current."""
        def worker():
            try:
                res = work()
            except Exception:  # pragma: no cover - shown as "no preview"
                res = None

            def back():
                if gen == self._gen:
                    done(res)
                return False
            GLib.idle_add(back)
        threading.Thread(target=worker, daemon=True, name="hb-preview").start()

    def _loading(self):
        sp = Gtk.Spinner(spinning=True, width_request=32, height_request=32,
                         halign=Gtk.Align.CENTER, valign=Gtk.Align.CENTER, vexpand=True)
        self.content.append(sp)
        return sp

    # ------------------------------------------------------------ update
    def update(self, tab):
        if not self.get_visible():
            return
        items = []
        kind = tab.kind() if tab is not None else None
        if tab is not None and kind not in ("home", "thispc"):
            items = tab.view.selected_items()
        key = (tab.location if tab else None,
               tuple(i.uri for i in items) if len(items) < 50 else len(items))
        if key == self._key:
            return
        self._key = key
        gen = self._clear()
        if not items:
            self._message("view-paged-symbolic", "Select a file to preview.")
            return
        if len(items) > 1:
            self._message("edit-select-all-symbolic", "%d items selected" % len(items))
            return
        self.show_item(items[0], gen)

    def reset(self):
        self._key = None
        self._clear()

    def show_item(self, item, gen=None):
        if gen is None:
            gen = self._clear()
        kind = fc.preview_kind(item.content_type, item.name, item.is_dir)
        path = item.path
        icon = item.icon or Gio.ThemedIcon.new("text-x-generic")
        if not path and kind != "none":
            kind = "none"
        if kind == "folder":
            box = self._message(icon, item.name, "Counting…")
            sub = box.get_last_child()
            self._later(gen, lambda: _count(path),
                        lambda n: sub.set_label(fc.item_count_text(n) if n is not None
                                                else "Can't read this folder"))
        elif kind in ("image", "pdf"):
            sp = self._loading()
            work = (lambda: _image_texture(path)) if kind == "image" else (lambda: _pdf_page(path))

            def got(pixbuf):
                self.content.remove(sp)
                if pixbuf is None:
                    self._message(icon, item.name, "No preview available")
                    return
                pic = Gtk.Picture(paintable=Gdk.Texture.new_for_pixbuf(pixbuf),
                                  can_shrink=True, vexpand=True,
                                  content_fit=Gtk.ContentFit.CONTAIN,
                                  css_classes=["hbf-preview-pic"])
                pic.set_alternative_text(item.name)
                self.content.append(pic)
                self.content.append(Gtk.Label(label=item.name, ellipsize=Pango.EllipsizeMode.MIDDLE,
                                              css_classes=["hb-dim"]))
            self._later(gen, work, got)
        elif kind == "text":
            sp = self._loading()

            def got_text(res):
                self.content.remove(sp)
                if res is None:
                    self._message(icon, item.name, "No preview available")
                    return
                text, truncated = res
                tv = Gtk.TextView(editable=False, cursor_visible=False, monospace=True,
                                  wrap_mode=Gtk.WrapMode.WORD_CHAR, top_margin=6, bottom_margin=6,
                                  left_margin=6, right_margin=6, css_classes=["hbf-preview-text"])
                tv.get_buffer().set_text(text)
                sw = Gtk.ScrolledWindow(child=tv, vexpand=True)
                self.content.append(sw)
                if truncated:
                    self.content.append(Gtk.Label(
                        label="Showing the first %d KB" % (fc.PREVIEW_TEXT_LIMIT // 1024),
                        css_classes=["hb-dim", "hb-small"]))
            self._later(gen, lambda: fc.read_text_preview(path), got_text)
        elif kind in ("video", "audio"):
            media = Gtk.MediaFile.new_for_file(item.gfile)
            video = Gtk.Video(media_stream=media, autoplay=False, loop=False, vexpand=True,
                              css_classes=["hbf-preview-video"])
            if kind == "audio":
                self._message(icon, item.name)
                video.set_vexpand(False)
                video.set_size_request(-1, 48)
            self.content.append(video)
        elif kind == "font":
            sp = self._loading()

            def got_family(family):
                self.content.remove(sp)
                if not family or not self._add_font(path):
                    self._message(icon, item.name, "No preview available")
                    return
                self.content.append(Gtk.Label(label=family, css_classes=["hbf-pane-title"],
                                              wrap=True))
                for size in (12, 18, 28, 40):
                    lab = Gtk.Label(label=FONT_SAMPLE if size < 28 else "Aa Bb Cc 123",
                                    wrap=True, xalign=0, selectable=True)
                    lab.set_wrap_mode(Pango.WrapMode.WORD_CHAR)
                    attrs = Pango.AttrList()
                    fd = Pango.FontDescription.from_string("%s %d" % (family, size))
                    attrs.insert(Pango.attr_font_desc_new(fd))
                    lab.set_attributes(attrs)
                    self.content.append(lab)
            self._later(gen, lambda: _font_family(path), got_family)
        else:
            self._message(icon, item.name, "No preview available")

    def _add_font(self, path):
        """Make a font file usable in this process (once); True on success."""
        if path in self._fonts_added:
            return True
        try:
            from gi.repository import PangoCairo
            ok = PangoCairo.FontMap.get_default().add_font_file(path)
        except (ImportError, AttributeError, GLib.Error, TypeError):
            return False
        if ok:
            self._fonts_added.add(path)
        return bool(ok)
