"""Harbor Files: Customize folder (colour, icon, emblem) stored as GIO metadata.

metadata::custom-icon is the key Nautilus uses too, so an imported picture
shows in both. Colour and emblem are Harbor's own keys. Nothing is written
in dry-run mode, and remote folders don't get the command.
"""

import hashlib
import os
import shutil

from gi.repository import Adw, Gdk, Gio, GLib, Gtk

from . import common
from . import files_core as fc
from . import files_icons as fi
from . import files_tint as tint

IMPORT_EXTS = (".png", ".svg", ".jpg", ".jpeg", ".webp", ".ico")
IMPORT_MAX = 5 * 1024 * 1024


def icons_dir():
    return os.path.join(GLib.get_user_data_dir(), "harbor", "folder-icons")


def import_icon(src, root=None):
    """Copy a picture into Harbor's icon store; return the stored path.
    Raises ValueError for a wrong type or a file over 5 MB."""
    ext = os.path.splitext(src)[1].lower()
    if ext not in IMPORT_EXTS:
        raise ValueError("Use a PNG, SVG, JPG, WebP or ICO picture")
    size = os.path.getsize(src)
    if size > IMPORT_MAX:
        raise ValueError("The picture is larger than 5 MB")
    h = hashlib.sha1()
    with open(src, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    root = root or icons_dir()
    dest = os.path.join(root, h.hexdigest() + (".jpg" if ext == ".jpeg" else ext))
    if not os.path.exists(dest):
        os.makedirs(root, exist_ok=True)
        tmp = dest + ".part"
        shutil.copyfile(src, tmp)
        os.chmod(tmp, 0o644)
        os.replace(tmp, dest)
    return dest


def metadata_info(icon_uri=None, icon_name=None, color=None, emblem=None):
    """Gio.FileInfo that sets the given values and clears the others."""
    info = Gio.FileInfo()
    vals = {fi.ATTR_CUSTOM_ICON: icon_uri, fi.ATTR_CUSTOM_ICON_NAME: icon_name,
            fi.ATTR_FOLDER_COLOR: color if tint.valid_hex(color) else None,
            fi.ATTR_EMBLEM: emblem if emblem in tint.EMBLEMS else None}
    for attr, v in vals.items():
        if v:
            info.set_attribute_string(attr, v)
        else:
            info.set_attribute(attr, Gio.FileAttributeType.INVALID, 0)   # 0 = unset
    return info


class CustomizeDialog(Adw.Dialog):
    def __init__(self, win, gfile):
        super().__init__(title="Customize folder", content_width=460)
        self.win = win
        self.gfile = gfile
        try:
            info = gfile.query_info(",".join(fi.METADATA_ATTRS), 0, None)
            uri, name, color, emblem = fi.custom_of(info)
        except GLib.Error:
            uri = name = color = emblem = None
        self.icon_uri, self.icon_name = uri, name
        self.color, self.emblem = color, emblem

        tv = Adw.ToolbarView()
        tv.add_top_bar(Adw.HeaderBar())
        page = Adw.PreferencesPage()
        tv.set_content(page)
        self.set_child(tv)

        # preview
        self.preview = Gtk.Image(pixel_size=96, margin_top=6)
        g0 = Adw.PreferencesGroup()
        g0.add(self.preview)
        page.add(g0)

        # colour
        gc = Adw.PreferencesGroup(title="Colour")
        flow = Gtk.FlowBox(selection_mode=Gtk.SelectionMode.NONE, max_children_per_line=11,
                           column_spacing=6, row_spacing=6)
        flow.append(self._swatch(None, "Default"))
        for key, hexv in fc.FOLDER_PALETTE.items():
            flow.append(self._swatch(hexv, key.title()))
        cbtn = Gtk.ColorDialogButton(dialog=Gtk.ColorDialog(with_alpha=False),
                                     tooltip_text="Custom colour", valign=Gtk.Align.CENTER)
        if tint.valid_hex(color):
            rgba = Gdk.RGBA()
            rgba.parse(color)
            cbtn.set_rgba(rgba)
        cbtn.connect("notify::rgba", self._on_custom_color)
        flow.append(cbtn)
        gc.add(flow)
        page.add(gc)

        # icon
        gi_ = Adw.PreferencesGroup(title="Icon")
        imp = Gtk.Button(label="Import picture…", css_classes=["flat"])
        imp.connect("clicked", self._import)
        gi_.set_header_suffix(imp)
        icons = Gtk.FlowBox(selection_mode=Gtk.SelectionMode.NONE, max_children_per_line=8,
                            column_spacing=4, row_spacing=4)
        for gname, label in tint.FOLDER_GLYPHS:
            b = Gtk.Button(tooltip_text=label, css_classes=["flat"],
                           child=Gtk.Image(icon_name=gname, pixel_size=40))
            b.connect("clicked", lambda _b, n=gname: self._set_icon(None, n))
            icons.append(b)
        gi_.add(icons)
        sysrow = Adw.EntryRow(title="System icon name (e.g. folder-music)",
                              show_apply_button=True)
        sysrow.connect("apply", lambda r: self._set_icon(None, r.get_text().strip() or None))
        gi_.add(sysrow)
        page.add(gi_)

        # emblem
        ge = Adw.PreferencesGroup(title="Badge")
        keys = [None] + list(tint.EMBLEMS)
        combo = Adw.ComboRow(title="Emblem", model=Gtk.StringList.new(
            ["None"] + [tint.EMBLEMS[k][0] for k in tint.EMBLEMS]))
        combo.set_selected(keys.index(emblem) if emblem in keys else 0)
        combo.connect("notify::selected",
                      lambda c, _p: (setattr(self, "emblem", keys[c.get_selected()]),
                                     self._refresh()))
        ge.add(combo)
        page.add(ge)

        # buttons
        gb = Adw.PreferencesGroup()
        bar = Gtk.Box(spacing=8, halign=Gtk.Align.END)
        reset = Gtk.Button(label="Restore default")
        reset.connect("clicked", lambda *_: self._save(True))
        ok = Gtk.Button(label="Apply", css_classes=["suggested-action"])
        ok.connect("clicked", lambda *_: self._save(False))
        bar.append(reset)
        bar.append(ok)
        gb.add(bar)
        page.add(gb)
        self._refresh()

    def _swatch(self, hexv, label):
        b = Gtk.Button(tooltip_text=label, width_request=28, height_request=28,
                       css_classes=["hbf-swatch", "circular"])
        if hexv:
            prov = Gtk.CssProvider()
            prov.load_from_string(".hbf-swatch { background: %s; }" % hexv)
            b.get_style_context().add_provider(prov, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        else:
            b.set_icon_name("edit-clear-symbolic")
        b.connect("clicked", lambda *_: (setattr(self, "color", hexv), self._refresh()))
        return b

    def _on_custom_color(self, btn, _p):
        c = btn.get_rgba()
        self.color = "#%02x%02x%02x" % (round(c.red * 255), round(c.green * 255),
                                        round(c.blue * 255))
        self._refresh()

    def _set_icon(self, uri, name):
        self.icon_uri, self.icon_name = uri, name
        self._refresh()

    def _import(self, *_):
        fd = Gtk.FileDialog(title="Choose a picture")
        flt = Gtk.FileFilter(name="Pictures")
        for e in IMPORT_EXTS:
            flt.add_suffix(e[1:])
        store = Gio.ListStore.new(Gtk.FileFilter)
        store.append(flt)
        fd.set_filters(store)

        def picked(d, res):
            try:
                g = d.open_finish(res)
            except GLib.Error:
                return
            if g is None or not g.get_path():
                return
            try:
                dest = g.get_path() if common.DRY_RUN else import_icon(g.get_path())
            except (ValueError, OSError) as e:
                self.win.notify_toast(str(e), timeout=5)
                return
            self._set_icon(Gio.File.new_for_path(dest).get_uri(), None)
        fd.open(self.win, None, picked)

    def _refresh(self):
        tmp = Gio.FileInfo()
        for attr, v in ((fi.ATTR_CUSTOM_ICON, self.icon_uri),
                        (fi.ATTR_CUSTOM_ICON_NAME, self.icon_name),
                        (fi.ATTR_FOLDER_COLOR, self.color), (fi.ATTR_EMBLEM, self.emblem)):
            if v:
                tmp.set_attribute_string(attr, v)
        g = fi.custom_gicon(tmp, True)
        if g is None:
            g = Gio.ThemedIcon.new(fi.tinted(fi.FOLDER, self.color, self.emblem))
        self.preview.set_from_gicon(g)
        if self.color or self.emblem:
            tint.request(self.color if tint.valid_hex(self.color) else fi.COLOR, self.emblem,
                         lambda: self.preview.set_from_gicon(
                             fi.custom_gicon(tmp, True) or Gio.ThemedIcon.new(
                                 fi.tinted(fi.FOLDER, self.color, self.emblem))))

    def _save(self, reset):
        info = metadata_info() if reset else metadata_info(self.icon_uri, self.icon_name,
                                                           self.color, self.emblem)
        if common.DRY_RUN:
            common.DRY_RUN_LOG.append(["customize", self.gfile.get_uri(), reset])
            self._done(None)
            return

        def finished(f, res):
            try:
                f.set_attributes_finish(res)
                self._done(None)
            except GLib.Error as e:
                self._done(e.message)
        self.gfile.set_attributes_async(info, Gio.FileQueryInfoFlags.NONE,
                                        GLib.PRIORITY_DEFAULT, None, finished)

    def _done(self, error):
        if error:
            self.win.notify_toast("Couldn't customize the folder: %s" % error, timeout=6)
            return
        tab = self.win.tab()
        if tab is not None:
            tab.refresh()
        self.close()
