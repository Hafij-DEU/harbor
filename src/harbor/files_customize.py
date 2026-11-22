"""Harbor Files: Customize folder (colour, icon, badge).

"Show this icon in" picks where the choice is kept:

* All file managers: GIO metadata (metadata::custom-icon / custom-icon-name),
  read by Files (Nautilus), Nemo, Caja and the desktop icons. Harbor glyphs
  and colours are exported to a standalone SVG for them.
* Harbor Files only: Harbor's private store; other apps keep their icon.

"Undo last change" puts back what was there before each change, in the
place it was made. Pictures are re-encoded copies, never the original file
(see files_iconstore). Nothing is written in dry-run mode, and remote
folders don't get the command.
"""

from gi.repository import Adw, Gdk, Gio, GLib, Gtk

from . import common
from . import files_core as fc
from . import files_iconstore as ics
from . import files_icons as fi
from . import files_tint as tint

IMPORT_EXTS = ics.PICTURE_EXTS


def import_icon(src, root=None):
    """Clean PNG copy of a picture in Harbor's icon store (see files_iconstore)."""
    return ics.import_picture(src, root)


def metadata_info(icon_uri=None, icon_name=None, color=None, emblem=None, glyph=None):
    """Gio.FileInfo that sets the given (checked) values and clears the others."""
    st = ics.clean_state({"icon_uri": icon_uri, "icon_name": icon_name,
                          "color": color, "emblem": emblem}) or {}
    info = Gio.FileInfo()
    vals = {fi.ATTR_CUSTOM_ICON: st.get("icon_uri"), fi.ATTR_CUSTOM_ICON_NAME: st.get("icon_name"),
            fi.ATTR_FOLDER_COLOR: st.get("color"), fi.ATTR_EMBLEM: st.get("emblem"),
            fi.ATTR_GLYPH: glyph if ics.valid_name(glyph) else None}
    for attr, v in vals.items():
        if v:
            info.set_attribute_string(attr, v)
        else:
            info.set_attribute(attr, Gio.FileAttributeType.INVALID, 0)   # 0 = unset
    return info


def global_editable(info):
    """What the global metadata means, as an editable state for the dialog.

    When Harbor exported a glyph, its name sits in metadata::harbor-icon-name,
    so the dialog reopens on the glyph and colour, not the exported file.
    """
    uri, name, color, emblem = fi.global_of(info)
    try:
        glyph = info.get_attribute_string(fi.ATTR_GLYPH) if info is not None else None
    except TypeError:
        glyph = None
    if glyph and ics.valid_name(glyph) and uri and uri.startswith(
            GLib.filename_to_uri(ics.icons_dir(), None)):
        uri, name = None, glyph
    return ics.clean_state({"icon_uri": uri, "icon_name": name, "color": color, "emblem": emblem})


def global_write_values(state):
    """(Gio.FileInfo to write, error or None) for a global state."""
    st = ics.clean_state(state) or {}
    try:
        uri, name = ics.global_values(st)
    except OSError as e:
        return None, "Couldn't save the icon: %s" % (e.strerror or e)
    glyph = None
    if uri and not st.get("icon_uri"):
        glyph = st.get("icon_name") or "harbor-folder"     # exported from a glyph
    return metadata_info(uri, name, st.get("color"), st.get("emblem"), glyph), None


class CustomizeDialog(Adw.Dialog):
    def __init__(self, win, gfile):
        super().__init__(title="Customize folder", content_width=480)
        self.win = win
        self.gfile = gfile
        self.path = gfile.get_path()
        self._quiet = False
        try:
            info = gfile.query_info(",".join(fi.METADATA_ATTRS), 0, None)
        except GLib.Error:
            info = None
        self.saved = {"global": global_editable(info),
                      "harbor": ics.store().get(self.path) if self.path else None}
        pref = win.settings.get("icon_scope") if hasattr(win, "settings") else None
        self.scope = ("harbor" if self.saved["harbor"]
                      else pref if pref in ics.SCOPES else "global")
        if not self.path:
            self.scope = "global"
        self._load_scope()

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

        # where
        gw = Adw.PreferencesGroup(title="Show this icon in")
        self.scope_rows = {}
        first = None
        for scope, title, sub in (
                ("global", "All file managers",
                 "Files, Nemo, Caja and the desktop show it too"),
                ("harbor", "Harbor Files only",
                 "Other file managers keep their own icon")):
            chk = Gtk.CheckButton(valign=Gtk.Align.CENTER, active=(scope == self.scope))
            if first is None:
                first = chk
            else:
                chk.set_group(first)
            row = Adw.ActionRow(title=title, subtitle=sub, activatable_widget=chk)
            row.add_prefix(chk)
            chk.connect("toggled", self._on_scope, scope)
            gw.add(row)
            self.scope_rows[scope] = chk
        page.add(gw)

        # colour
        gc = Adw.PreferencesGroup(title="Colour")
        flow = Gtk.FlowBox(selection_mode=Gtk.SelectionMode.NONE, max_children_per_line=11,
                           column_spacing=6, row_spacing=6)
        flow.append(self._swatch(None, "Default"))
        for key, hexv in fc.FOLDER_PALETTE.items():
            flow.append(self._swatch(hexv, key.title()))
        self.cbtn = Gtk.ColorDialogButton(dialog=Gtk.ColorDialog(with_alpha=False),
                                          tooltip_text="Custom colour", valign=Gtk.Align.CENTER)
        self._sync_color_button()
        self.cbtn.connect("notify::rgba", self._on_custom_color)
        flow.append(self.cbtn)
        gc.add(flow)
        page.add(gc)

        # icon
        gi_ = Adw.PreferencesGroup(
            title="Icon",
            description="A picture from anywhere on this computer (PNG, SVG, JPG, WebP, ICO, "
                        "GIF, BMP, AVIF or JPEG XL, up to 5 MB). Harbor keeps its own safe copy.")
        imp = Gtk.Button(label="Choose picture…", css_classes=["flat"])
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
        self.sysrow = Adw.EntryRow(title="Icon from the system theme (e.g. folder-music)",
                                   show_apply_button=True)
        self.sysrow.connect("apply", self._on_system_name)
        gi_.add(self.sysrow)
        page.add(gi_)

        # badge
        ge = Adw.PreferencesGroup(title="Badge")
        self._badges = [None] + list(tint.EMBLEMS)
        self.badge = Adw.ComboRow(title="Emblem", model=Gtk.StringList.new(
            ["None"] + [tint.EMBLEMS[k][0] for k in tint.EMBLEMS]))
        self._sync_badge()
        self.badge.connect("notify::selected", self._on_badge)
        ge.add(self.badge)
        page.add(ge)

        # buttons
        gb = Adw.PreferencesGroup()
        bar = Gtk.Box(spacing=8)
        self.undo_btn = Gtk.Button(label="Undo last change",
                                   tooltip_text="Put back the icon this folder had before")
        self.undo_btn.connect("clicked", self._undo)
        bar.append(self.undo_btn)
        bar.append(Gtk.Box(hexpand=True))
        self.reset_btn = Gtk.Button(label="Restore default")
        self.reset_btn.connect("clicked", lambda *_: self._apply(reset=True))
        ok = Gtk.Button(label="Apply", css_classes=["suggested-action"])
        ok.connect("clicked", lambda *_: self._apply(reset=False))
        bar.append(self.reset_btn)
        bar.append(ok)
        gb.add(bar)
        page.add(gb)
        self._refresh()

    # ------------------------------------------------------------ state

    def _load_scope(self):
        st = self.saved.get(self.scope) or {}
        self.icon_uri = st.get("icon_uri")
        self.icon_name = st.get("icon_name")
        self.color = st.get("color")
        self.emblem = st.get("emblem")

    def _state(self):
        return ics.clean_state({"icon_uri": self.icon_uri, "icon_name": self.icon_name,
                                "color": self.color, "emblem": self.emblem})

    def _on_scope(self, chk, scope):
        if not chk.get_active() or scope == self.scope:
            return
        if scope == "harbor" and not self.path:
            self.win.notify_toast("Only local folders can have a Harbor-only icon", timeout=5)
            self.scope_rows["global"].set_active(True)
            return
        self.scope = scope
        self._load_scope()
        self._sync_color_button()
        self._sync_badge()
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

    def _sync_color_button(self):
        if not hasattr(self, "cbtn") or not tint.valid_hex(self.color):
            return
        self._quiet = True
        rgba = Gdk.RGBA()
        rgba.parse(self.color)
        self.cbtn.set_rgba(rgba)
        self._quiet = False

    def _sync_badge(self):
        if not hasattr(self, "badge"):
            return
        self._quiet = True
        self.badge.set_selected(self._badges.index(self.emblem) if self.emblem in self._badges else 0)
        self._quiet = False

    def _on_badge(self, combo, _p):
        if self._quiet:
            return
        self.emblem = self._badges[combo.get_selected()]
        self._refresh()

    def _on_custom_color(self, btn, _p):
        if self._quiet:
            return
        c = btn.get_rgba()
        self.color = "#%02x%02x%02x" % (round(c.red * 255), round(c.green * 255),
                                        round(c.blue * 255))
        self._refresh()

    def _on_system_name(self, row):
        name = row.get_text().strip()
        if name and not ics.valid_name(name):
            self.win.notify_toast("Use an icon name like folder-music (letters, digits, - . _ +)",
                                  timeout=5)
            return
        self._set_icon(None, name or None)

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
                self.win.notify_toast("Pick a picture on this computer", timeout=5)
                return
            try:
                dest = import_icon(g.get_path(), root=self._dry_root())
            except (ValueError, OSError) as e:
                self.win.notify_toast(str(e), timeout=5)
                return
            self._set_icon(GLib.filename_to_uri(dest, None), None)
        fd.open(self.win, None, picked)

    def _dry_root(self):
        """Dry run: keep pictures in a throwaway folder, not the real store."""
        if not common.DRY_RUN:
            return None
        import tempfile
        if not getattr(self, "_tmp", None):
            self._tmp = tempfile.mkdtemp(prefix="harbor-icons-")
        return self._tmp

    def _refresh(self):
        st = self._state() or {}
        tmp = Gio.FileInfo()
        for attr, key in ((fi.ATTR_CUSTOM_ICON, "icon_uri"), (fi.ATTR_CUSTOM_ICON_NAME, "icon_name"),
                          (fi.ATTR_FOLDER_COLOR, "color"), (fi.ATTR_EMBLEM, "emblem")):
            if st.get(key):
                tmp.set_attribute_string(attr, st[key])

        def show():
            g = fi.custom_gicon(tmp, True)
            if g is None:
                g = Gio.ThemedIcon.new(fi.tinted(fi.FOLDER, self.color, self.emblem))
            self.preview.set_from_gicon(g)
        show()
        if self.color or self.emblem:
            tint.request(self.color if tint.valid_hex(self.color) else fi.COLOR, self.emblem, show)
        if hasattr(self, "undo_btn"):
            self.undo_btn.set_sensitive(bool(self.path) and ics.store().can_undo(self.path))
            self.reset_btn.set_sensitive(bool(self.saved.get(self.scope)) or bool(st))

    # ------------------------------------------------------------ saving

    def _apply(self, reset):
        state = None if reset else self._state()
        try:
            self.win.settings.set("icon_scope", self.scope)
        except Exception:   # noqa: BLE001 - a settings hiccup must not lose the change
            pass
        self._write(self.scope, state, record=True)

    def _undo(self, *_):
        if not self.path:
            return
        step = ics.store().pop(self.path)
        if step is None:
            self._refresh()
            return
        self._write(step["scope"], step["state"], record=False, undo_step=step)

    def _write(self, scope, state, record, undo_step=None):
        store = ics.store()
        previous = self.saved.get(scope)
        if common.DRY_RUN:
            common.DRY_RUN_LOG.append(["customize", self.gfile.get_uri(), scope, state])
            if undo_step is not None:       # nothing was undone for real
                store.history.setdefault(ics.key_for(self.path), []).append(undo_step)
            self._done(None, scope, state)
            return
        if scope == "harbor":
            try:
                if record:
                    store.push(self.path, "harbor", previous)
                store.put(self.path, state)
                store.save()
            except (OSError, ValueError) as e:
                if record:
                    store.pop(self.path)
                self._failed(store, undo_step, str(e))
                return
            self._done(None, scope, state)
            return

        info, err = global_write_values(state)
        if err:
            self._failed(store, undo_step, err)
            return

        def finished(f, res):
            try:
                f.set_attributes_finish(res)
            except GLib.Error as e:
                self._failed(store, undo_step, e.message)
                return
            if self.path:
                try:
                    if record:
                        store.push(self.path, "global", previous)
                    store.save()
                except OSError:
                    pass        # the icon changed; only the undo step was not kept
            self._done(None, scope, state)
        self.gfile.set_attributes_async(info, Gio.FileQueryInfoFlags.NONE,
                                        GLib.PRIORITY_DEFAULT, None, finished)

    def _failed(self, store, undo_step, message):
        if undo_step is not None and self.path:     # keep the step so undo can be retried
            store.history.setdefault(ics.key_for(self.path), []).append(undo_step)
        self._done(message, None, None)

    def _done(self, error, scope, state):
        if error:
            self.win.notify_toast("Couldn't customize the folder: %s" % error, timeout=6)
            self._refresh()
            return
        self.saved[scope] = ics.clean_state(state)
        where = "in Harbor Files only" if scope == "harbor" else "in all file managers"
        self.win.notify_toast("Folder icon updated %s" % where)
        tab = self.win.tab()
        if tab is not None:
            tab.refresh()
        self.close()
