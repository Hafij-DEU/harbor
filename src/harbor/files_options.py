"""Harbor Files: the Settings window.

Open it with the gear in the header, Ctrl+comma, "… > Settings" or File >
Options in the ribbon. Every row writes fc.Settings through
FilesWindow.apply_setting(), which applies the change live to every open
window and saves it.
"""

import os

from gi.repository import Adw, Gdk, GLib, Gtk

from . import VERSION, common
from . import files_core as fc
from . import files_office

REPO_URL = "https://github.com/Hafij-DEU/harbor"
AUTHOR = "Md. Hafijur Rahman"

PALETTE_LABELS = {"blue": "Blue", "teal": "Teal", "green": "Green", "yellow": "Yellow",
                  "orange": "Orange", "red": "Red", "pink": "Pink", "purple": "Purple",
                  "slate": "Slate"}

# (settings key, title, subtitle, [(value, label)]) - the values must match
# fc.Settings.CHOICES (checked by the tests)
APPEARANCE_CHOICES = (
    ("style", "Style", "System uses your GNOME colours and accent",
     (("win11", "Windows 11"), ("system", "System"))),
    ("theme", "Theme", "Light or dark for Harbor only; Follow system uses GNOME's setting",
     (("follow", "Follow system"), ("light", "Light"), ("dark", "Dark"))),
    ("glass", "Glass effect", "Style, opacity, rim and glare: Harbor Settings > Personalisation",
     (("harbor", "Same as all Harbor apps"), ("off", "Off"), ("soft", "Soft"),
      ("strong", "Strong"), ("planner", "Lumina Glass"), ("custom", "Custom (Harbor Settings)"))),
    ("icons", "Icons", "Modern uses Harbor's own icon set (glossy with Lumina Glass)",
     (("modern", "Modern"), ("glossy", "Glossy"), ("system", "System icon theme"))),
    ("folder_color", "Folder colour", "Every folder icon in the Modern set",
     (("classic", "Classic yellow"), ("accent", "Follow accent colour"))
     + tuple((k, PALETTE_LABELS[k]) for k in fc.FOLDER_PALETTE) + (("custom", "Custom"),)),
)
STARTUP_CHOICES = (
    ("open_to", "Open Harbor Files to", "What a new window shows first",
     (("thispc", "This PC"), ("home", "Home"), ("last", "Last folder"))),
)
LAYOUT_CHOICES = (
    ("density", "Layout density", "Compact fits more rows on screen",
     (("comfortable", "Comfortable"), ("compact", "Compact"))),
    ("menu_style", "Right-click menu", "Show more options (Shift+F10) always opens the classic one",
     (("win11", "Windows 11"), ("classic", "Classic (Windows 10)"),
      ("merged", "Merged (Windows 10 + 11)"))),
)
BEHAVIOUR_CHOICES = (
    ("click_to_open", "Open items with", None,
     (("double", "Double-click"), ("single", "Single click"))),
)
# kept for older callers
GENERAL_CHOICES = STARTUP_CHOICES + LAYOUT_CHOICES
VIEW_CHOICES = APPEARANCE_CHOICES

# rows that are not plain settings keys
ICON_SIZES = (("large", "Large icons"), ("medium", "Medium icons"))
DEFAULT_VIEWS = (("details", "Details"), ("large", "Large icons"), ("medium", "Medium icons"))
START_PANES = (("none", "None"), ("details", "Details pane"), ("preview", "Preview pane"))
SORT_LABELS = (("name", "Name"), ("modified", "Date modified"), ("type", "Type"), ("size", "Size"))
SORT_ORDERS = (("asc", "Ascending"), ("desc", "Descending"))
PREVIEW_SIZES = (("small", "Small (320 px)"), ("medium", "Medium (420 px)"),
                 ("large", "Large (560 px)"))

SWITCHES = {
    "restore_tabs": ("Restore tabs", "Reopen the tabs of the last window"),
    "show_status_bar": ("Status bar", "Item count and selection size at the bottom"),
    "confirm_trash": ("Ask before moving to the Trash", None),
    "show_hidden": ("Show hidden items", None),
    "show_extensions": ("Show file name extensions", None),
    "show_quick_access": ("Show Quick access", "Pinned folders in the navigation pane and Home"),
    "show_recent": ("Show recent files in Home", None),
    "share_in_sidebar": ("Share over Wi-Fi in the navigation pane",
                         "It is always in the Share button (and the ribbon's Share tab)"),
    "usb_in_sidebar": ("Phone over USB cable in the navigation pane",
                       "Opens the share window, where Connect sends this PC to a phone on the cable"),
    "open_archives_in_harbor": ("Open archives in Harbor",
                                "Double-click lists a .zip / .7z / .tar here instead of the "
                                "default app"),
    "folder_sizes": ("Folder sizes", "Fill in the Size column for folders in the background"),
    "show_storage": ("Storage overview button",
                     "Small drive button next to the view buttons: total, used and free space"),
    "image_thumbnails": ("Image thumbnails", None),
    "doc_thumbnails": ("Document thumbnails", "Office pages in Large icons (slower)"),
    "office_preview": ("Office preview", None),
    "remote_thumbnails": ("Thumbnails on network locations", "Downloads files to make them"),
}


def _all_choices():
    return APPEARANCE_CHOICES + STARTUP_CHOICES + LAYOUT_CHOICES + BEHAVIOUR_CHOICES


DESIGNS = (("win11", "Windows 11", "Tabs in the title bar, command bar, rounded"),
           ("win10", "Windows 10", "Blue title bar, File / Home / Share / View ribbon, flat"),
           ("custom", "Custom", "Harbor's own: modern ribbon, floating rounded panels, pill controls"))
ACCENTS = (("#3584e4", "Blue"), ("#2190a4", "Teal"), ("#3a944a", "Green"),
           ("#c88800", "Yellow"), ("#ed5b00", "Orange"), ("#e62d42", "Red"),
           ("#d56199", "Pink"), ("#9141ac", "Purple"), ("#6f8396", "Slate"))


def design_thumb(value, width=150, height=84):
    """A tiny drawing of the window made of boxes (no images needed)."""
    t = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, css_classes=["hbo-thumb"],
                height_request=height, width_request=width)
    if value == "win10":
        t.append(Gtk.Box(css_classes=["hbo-t-blue"]))
        tabs = Gtk.Box(css_classes=["hbo-t-tabs"])
        tabs.append(Gtk.Box(css_classes=["hbo-t-tab"]))
        t.append(tabs)
        t.append(Gtk.Box(css_classes=["hbo-t-ribbon"]))
        t.append(Gtk.Box(css_classes=["hbo-t-row"]))
    elif value == "custom":
        tabs = Gtk.Box(css_classes=["hbo-t-tabs"])
        tabs.append(Gtk.Box(css_classes=["hbo-t-tab", "hbo-t-pill"]))
        t.append(tabs)
        t.append(Gtk.Box(css_classes=["hbo-t-ribbon", "hbo-t-cmdcard", "hbo-t-ribcard"]))
        t.append(Gtk.Box(css_classes=["hbo-t-row", "hbo-t-pill"]))
    else:
        tabs = Gtk.Box(css_classes=["hbo-t-tabs"])
        tabs.append(Gtk.Box(css_classes=["hbo-t-tab"]))
        t.append(tabs)
        t.append(Gtk.Box(css_classes=["hbo-t-row", "hbo-t-round"]))
        t.append(Gtk.Box(css_classes=["hbo-t-cmd"]))
    if value == "custom":
        body = Gtk.Box(vexpand=True, spacing=4, css_classes=["hbo-t-float"])
        body.append(Gtk.Box(css_classes=["hbo-t-side", "hbo-t-card"]))
        body.append(Gtk.Box(hexpand=True, css_classes=["hbo-t-card"]))
        t.add_css_class("hbo-thumb-w12")
    else:
        body = Gtk.Box(vexpand=True, css_classes=["hbo-t-body"])
        body.append(Gtk.Box(css_classes=["hbo-t-side"]))
    t.append(body)
    if value == "win11":
        t.add_css_class("hbo-thumb-round")
    return t


def _check_choices():
    """Every choice row matches fc.Settings.CHOICES (used by the tests)."""
    for key, _t, _s, opts in _all_choices():
        assert tuple(v for v, _l in opts) == fc.Settings.CHOICES[key], key
    for key in SWITCHES:
        assert key in fc.Settings.DEFAULTS, key
    assert tuple(v for v, _l in PREVIEW_SIZES) == tuple(fc.PREVIEW_WIDTHS)
    assert tuple(v for v, _l in SORT_LABELS) == fc.SORT_FIELDS
    return True


def start_pane(settings):
    if settings.get("preview_pane"):
        return "preview"
    return "details" if settings.get("details_pane") else "none"


def default_view(settings):
    return "details" if settings.get("view") == "details" else settings.get("icon_size")


def office_status(tools):
    """Subtitle for the Office preview row from files_office.tools()."""
    have = [files_office.APP_NAMES[k] for k in ("word", "slides", "sheet") if tools.get(k)]
    if len(have) == 3:
        return "Craft Suite found: Word, Slide and Spreadsheet pages are drawn"
    if have:
        return "Craft Suite: %s only; the others show text" % ", ".join(have)
    return "Craft Suite not found: shows the built-in thumbnail or the text"


class SettingsDialog(Adw.PreferencesDialog):
    def __init__(self, win, page=None):
        super().__init__(title="Settings", search_enabled=True)
        self.win = win
        self.s = win.settings
        self.rows = {}
        self._build_appearance()
        self._build_startup()
        self._build_layout()
        self._build_behaviour()
        self._build_preview()
        self._build_default_apps()
        self._build_about()
        if page:
            self.set_visible_page_name(page)
        self._fm_state = "unknown"
        self.refresh_fm()
        self.refresh_cache()
        self.refresh_wifi()

    # ------------------------------------------------------------ pages
    def _page(self, name, title, icon):
        p = Adw.PreferencesPage(name=name, title=title, icon_name=icon)
        self.add(p)
        return p

    def _build_appearance(self):
        p = self._page("appearance", "Appearance", "applications-graphics-symbolic")
        p.add(self._design_group())
        g = Adw.PreferencesGroup(title="Look")
        for spec in APPEARANCE_CHOICES:
            g.add(self._combo(*spec))
        self.custom_row = Adw.ActionRow(title="Custom folder colour")
        self.color_btn = Gtk.ColorDialogButton(dialog=Gtk.ColorDialog(with_alpha=False),
                                               valign=Gtk.Align.CENTER)
        rgba = Gdk.RGBA()
        rgba.parse(self.s.get("folder_color_custom") or "#9141ac")
        self.color_btn.set_rgba(rgba)
        self.color_btn.connect("notify::rgba", self._custom_color)
        self.custom_row.add_suffix(self.color_btn)
        self.custom_row.set_activatable_widget(self.color_btn)
        g.add(self.custom_row)
        self._sync_custom()
        g.add(self._value_combo("icon_size", "Default icon size", None, ICON_SIZES,
                                self.s.get("icon_size"), self._set_icon_size))
        g.add(self._accent_row())
        p.add(g)
        w = Adw.PreferencesGroup(title="Welcome")
        row = Adw.ActionRow(title="Welcome tour and setup",
                            subtitle="See what Harbor Files can do and pick your design again")
        btn = Gtk.Button(label="Open", valign=Gtk.Align.CENTER)
        btn.connect("clicked", self._open_welcome)
        row.add_suffix(btn)
        row.set_activatable_widget(btn)
        w.add(row)
        p.add(w)

    def _open_welcome(self, *_):
        from . import files_welcome
        win = self.win
        self.close()
        files_welcome.show(win)

    def _accent_row(self):
        row = Adw.ActionRow(title="Accent colour",
                            subtitle="Buttons, selection and highlights in Harbor")
        box = Gtk.Box(spacing=4, valign=Gtk.Align.CENTER)
        self.accent_btns = accent_swatches(self.s.get("accent"),
                                           lambda v: self.win.apply_setting("accent", v), box)
        row.add_suffix(box)
        return row

    def _build_startup(self):
        p = self._page("startup", "Start-up", "media-playback-start-symbolic")
        g = Adw.PreferencesGroup(title="New windows")
        for spec in STARTUP_CHOICES:
            g.add(self._combo(*spec))
        g.add(self._switch("restore_tabs"))
        p.add(g)

    def _build_layout(self):
        p = self._page("layout", "Layout", "view-dual-symbolic")
        g = Adw.PreferencesGroup(title="Bars and menus")
        for spec in LAYOUT_CHOICES:
            g.add(self._combo(*spec))
        g.add(self._switch("show_status_bar"))
        g.add(self._switch("share_in_sidebar"))
        g.add(self._switch("folder_sizes"))
        g.add(self._switch("show_storage"))
        p.add(g)
        g2 = Adw.PreferencesGroup(title="Views")
        g2.add(self._value_combo("start_pane", "Pane at start-up",
                                 "Details and preview share one place", START_PANES,
                                 start_pane(self.s), self._set_pane))
        g2.add(self._value_combo("default_view", "Default view", None, DEFAULT_VIEWS,
                                 default_view(self.s), self._set_view))
        g2.add(self._value_combo("sort", "Sort by", None, SORT_LABELS, self.s.get("sort"),
                                 lambda v: self.win.activate_action("win.sort",
                                                                    GLib.Variant("s", v))))
        g2.add(self._value_combo("sort_order", "Sort direction", None, SORT_ORDERS,
                                 "desc" if self.s.get("descending") else "asc",
                                 lambda v: self.win.activate_action("win.sort-order",
                                                                    GLib.Variant("s", v))))
        g2.add(self._value_combo("preview_width", "Preview pane size",
                                 "You can also drag the divider", PREVIEW_SIZES,
                                 fc.preview_size_name(self.s.get("preview_width")),
                                 lambda v: self.win.apply_setting("preview_width",
                                                                  fc.PREVIEW_WIDTHS[v])))
        p.add(g2)

    def _build_behaviour(self):
        p = self._page("behaviour", "Behaviour", "input-mouse-symbolic")
        g = Adw.PreferencesGroup(title="Opening and deleting")
        for spec in BEHAVIOUR_CHOICES:
            g.add(self._combo(*spec))
        g.add(self._switch("confirm_trash"))
        perm = Adw.SwitchRow(title="Ask before deleting permanently",
                             subtitle="Always on: a permanent delete cannot be undone",
                             active=True, sensitive=False)
        g.add(perm)
        g.add(self._switch("open_archives_in_harbor"))
        p.add(g)
        g2 = Adw.PreferencesGroup(title="Files and folders")
        for key in ("show_hidden", "show_extensions", "show_quick_access", "show_recent"):
            g2.add(self._switch(key))
        p.add(g2)
        g3 = Adw.PreferencesGroup(title="Network")
        self.wifi_row = Adw.SwitchRow(
            title="Wi-Fi power saving",
            subtitle="Off keeps phone and network folders quick. Uses a little more battery.",
            sensitive=False)
        self._wifi_conns = []
        self._wifi_sync = False
        self.wifi_row.connect("notify::active", self._wifi_toggled)
        g3.add(self.wifi_row)
        g3.add(self._combo("remote_media", "Big phone videos and music",
                           "Videos over 5 MB or music over 50 MB opened from the phone. Downloads are deleted when the player closes them",
                           [("ask", "Ask each time"), ("stream", "Stream"),
                            ("download", "Download first")]))
        p.add(g3)
        g4 = Adw.PreferencesGroup(
            title="USB cable",
            description="A phone with Harbor Files opens this PC's shared folders through the "
                        "cable: no tethering, no Wi-Fi, no developer options. While connected, "
                        "the PC can't see the phone's own files over USB.")
        g4.add(self._combo("share_usb", "Share over USB cable",
                           "Used the next time sharing starts",
                           [("ask", "When I click Connect"),
                            ("auto", "Every phone plugged in"),
                            ("only", "USB cable only (not on Wi-Fi)"),
                            ("off", "Off")]))
        g4.add(self._switch("usb_in_sidebar"))
        p.add(g4)

    def _build_preview(self):
        p = self._page("preview", "Preview", "view-paged-symbolic")
        g = Adw.PreferencesGroup(title="Thumbnails")
        for key in ("image_thumbnails", "doc_thumbnails", "remote_thumbnails"):
            g.add(self._switch(key))
        p.add(g)
        g2 = Adw.PreferencesGroup(title="Preview pane")
        row = self._switch("office_preview")
        row.set_subtitle(office_status(files_office.tools()))
        g2.add(row)
        spin = Adw.SpinRow.new_with_range(1, 2048, 1)
        spin.set_title("Largest file to preview")
        spin.set_subtitle("In MB; bigger files show their icon")
        spin.set_value(float(self.s.get("preview_max_mb") or 50))
        spin.connect("notify::value",
                     lambda r, _p: self.win.apply_setting("preview_max_mb", int(r.get_value())))
        self.rows["preview_max_mb"] = spin
        g2.add(spin)
        self.cache_row = Adw.ActionRow(title="Preview cache", subtitle="Measuring…")
        self.cache_btn = Gtk.Button(label="Clear preview cache", valign=Gtk.Align.CENTER,
                                    sensitive=False)
        self.cache_btn.connect("clicked", self._clear_cache)
        self.cache_row.add_suffix(self.cache_btn)
        g2.add(self.cache_row)
        p.add(g2)

    def _build_default_apps(self):
        p = self._page("default-apps", "Default apps", "application-x-executable-symbolic")
        g = Adw.PreferencesGroup(title="Default file manager",
                                 description="Folders opened from other apps use the default "
                                             "file manager. Nothing changes until you click.")
        self.fm_row = Adw.ActionRow(title="Harbor Files", subtitle="Checking…")
        self.fm_btn = Gtk.Button(label="Make default", valign=Gtk.Align.CENTER, sensitive=False)
        self.fm_btn.connect("clicked", self._fm_clicked)
        self.fm_row.add_suffix(self.fm_btn)
        g.add(self.fm_row)
        p.add(g)

    def _build_about(self):
        p = self._page("about", "About", "help-about-symbolic")
        g = Adw.PreferencesGroup(title="Harbor Files")
        for title, sub in (("Version", VERSION), ("Licence", "MIT"), ("Author", AUTHOR)):
            r = Adw.ActionRow(title=title, subtitle=sub, subtitle_selectable=True)
            r.add_css_class("property")
            g.add(r)
        link = Adw.ActionRow(title="Source code", subtitle=REPO_URL, activatable=True)
        link.add_suffix(Gtk.Image(icon_name="adw-external-link-symbolic"))
        link.connect("activated", lambda *_: Gtk.UriLauncher(uri=REPO_URL).launch(
            self.win, None, None, None))
        g.add(link)
        p.add(g)

    # ------------------------------------------------------------ rows
    def _combo(self, key, title, subtitle, opts):
        return self._value_combo(key, title, subtitle, opts, self.s.get(key),
                                 lambda v: self.win.apply_setting(key, v))

    def _value_combo(self, key, title, subtitle, opts, current, apply):
        values = [v for v, _l in opts]
        row = Adw.ComboRow(title=title, model=Gtk.StringList.new([lab for _v, lab in opts]))
        if subtitle:
            row.set_subtitle(subtitle)
        row.set_selected(values.index(current) if current in values else 0)
        row.values = values
        row.syncing = False

        def changed(r, _p):
            if r.syncing:
                return
            apply(values[r.get_selected()])
            if key == "folder_color":
                self._sync_custom()
        row.connect("notify::selected", changed)
        self.rows[key] = row
        return row

    def _switch(self, key):
        title, subtitle = SWITCHES[key]
        row = Adw.SwitchRow(title=title, active=bool(self.s.get(key)))
        if subtitle:
            row.set_subtitle(subtitle)
        row.connect("notify::active", lambda r, _p: self.win.apply_setting(key, r.get_active()))
        self.rows[key] = row
        return row

    def _design_group(self):
        g = Adw.PreferencesGroup(title="Interface style",
                                 description="Changes the whole window layout, not only colours")
        box = Gtk.Box(spacing=12, homogeneous=True)
        self.design_btns = {}
        first = None
        for value, label, sub in DESIGNS:
            card = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6, margin_top=8,
                           margin_bottom=8, margin_start=8, margin_end=8)
            card.append(design_thumb(value))
            card.append(Gtk.Label(label=label, css_classes=["heading"]))
            card.append(Gtk.Label(label=sub, css_classes=["dim-label", "caption"], wrap=True,
                                  justify=Gtk.Justification.CENTER))
            b = Gtk.ToggleButton(child=card, css_classes=["hbo-design"],
                                 active=self.s.get("ui_design") == value)
            if first is None:
                first = b
            else:
                b.set_group(first)
            b.connect("toggled", lambda w, v=value: w.get_active()
                      and self.s.get("ui_design") != v and self.win.apply_setting("ui_design", v))
            self.design_btns[value] = b
            box.append(b)
        g.add(box)
        return g

    def sync(self, key, value):
        """Reflect a change made elsewhere (menu, ribbon, keyboard)."""
        if key == "ui_design":
            b = self.design_btns.get(value)
            if b is not None and not b.get_active():
                b.set_active(True)
            return
        if key == "descending":
            key, value = "sort_order", "desc" if value else "asc"
        elif key in ("view", "icon_size"):
            row = self.rows.get("default_view")
            if row is not None:
                self._select(row, default_view(self.s))
            if key == "view":
                return
        elif key in ("details_pane", "preview_pane"):
            key, value = "start_pane", start_pane(self.s)
        row = self.rows.get(key)
        if isinstance(row, Adw.SwitchRow):
            if row.get_active() != bool(value):
                row.set_active(bool(value))
        elif isinstance(row, Adw.ComboRow):
            self._select(row, value)

    @staticmethod
    def _select(row, value):
        if value in row.values and row.values[row.get_selected()] != value:
            row.syncing = True
            row.set_selected(row.values.index(value))
            row.syncing = False

    # ------------------------------------------------------------ special rows
    def _sync_custom(self):
        self.custom_row.set_visible(self.s.get("folder_color") == "custom")

    def _custom_color(self, btn, _p):
        c = btn.get_rgba()
        value = "#%02x%02x%02x" % tuple(round(x * 255) for x in (c.red, c.green, c.blue))
        self.win.apply_setting("folder_color_custom", value)

    def _set_icon_size(self, size):
        self.s.set("icon_size", size)
        if self.s.get("view") != "details":
            self.win.activate_action("win.view", GLib.Variant("s", size))

    def _set_view(self, key):
        self.win.activate_action("win.view", GLib.Variant("s", key))

    def _set_pane(self, value):
        if value == "none":
            for a in ("win.preview-pane", "win.details-pane"):
                self.win.activate_action(a, GLib.Variant("b", False))
        else:
            self.win.activate_action("win.%s-pane" % value, GLib.Variant("b", True))

    # ------------------------------------------------------------ preview cache
    def refresh_cache(self):
        fc.run_in_thread(lambda: files_office.cache_size(), self._cache_got)

    def _cache_got(self, size):
        size = size if isinstance(size, int) else 0
        where = files_office.cache_dir().replace(os.path.expanduser("~"), "~", 1)
        self.cache_row.set_subtitle("%s in %s" % (common.human_size(size), where))
        self.cache_btn.set_sensitive(size > 0)

    def _clear_cache(self, _b):
        self.cache_btn.set_sensitive(False)
        if common.DRY_RUN:
            self.win.notify_toast("Dry run: would clear the preview cache")
            return
        fc.run_in_thread(lambda: files_office.clear_cache(), lambda *_: self.refresh_cache())

    # ------------------------------------------------------------ default file manager
    def refresh_fm(self):
        common.run_cmd(fc.default_fm_query_argv(), self._fm_got, mutating=False)

    def _fm_got(self, ok, out, _err):
        state = fc.default_fm_state(out if ok else "")
        self._fm_state = state
        current = (out or "").strip().splitlines()[0] if (ok and (out or "").strip()) else ""
        if state == "harbor":
            self.fm_row.set_subtitle("Harbor Files is the default (%s)" % current)
            self.fm_btn.set_label("Restore Nautilus")
        else:
            self.fm_row.set_subtitle("Current default: %s" % (current or "not set"))
            self.fm_btn.set_label("Make default")
        self.fm_btn.set_sensitive(True)

    def _fm_clicked(self, _b):
        target = fc.NAUTILUS_DESKTOP if self._fm_state == "harbor" else fc.FILES_DESKTOP
        self.fm_btn.set_sensitive(False)

        def done(ok, _out, err):
            if common.DRY_RUN:
                self.win.notify_toast("Dry run: would set %s as the default" % target)
            elif not ok:
                self.win.notify_toast("Can't change the default: %s" % (err.strip() or "error"))
            self.refresh_fm()
        common.run_cmd(fc.default_fm_set_argv(target), done)

    # ------------------------------------------------------------ Wi-Fi power saving
    def refresh_wifi(self):
        common.run_cmd(fc.wifi_ps_query_argv(), self._wifi_got, mutating=False)

    def _wifi_got(self, ok, out, _err):
        state = fc.wifi_ps_state(out if ok else "")
        saved = fc.wifi_ps_saved_on(state)
        self._wifi_conns = list(state["conns"])
        if saved is None:
            self.wifi_row.set_sensitive(False)
            self.wifi_row.set_subtitle("No Wi-Fi networks saved")
            return
        self._wifi_sync = True
        self.wifi_row.set_active(saved)
        self._wifi_sync = False
        self.wifi_row.set_sensitive(True)
        live = state["live"]
        if live and any(v != saved for v in live.values()):
            self.wifi_row.set_subtitle("Saved %s. Takes effect when Wi-Fi reconnects."
                                       % ("on" if saved else "off"))

    def _wifi_toggled(self, row, _p):
        if self._wifi_sync or not self._wifi_conns:
            return
        on = row.get_active()
        row.set_sensitive(False)

        def done(ok, _out, err):
            if common.DRY_RUN:
                self.win.notify_toast("Dry run: would turn Wi-Fi power saving %s"
                                      % ("on" if on else "off"))
            elif not ok:
                self.win.notify_toast("Can't change Wi-Fi power saving: %s"
                                      % (err.strip() or "error"))
            self.refresh_wifi()
        common.run_cmd(fc.wifi_ps_set_argv(self._wifi_conns, on), done)


OptionsDialog = SettingsDialog


def accent_swatches(current, on_pick, box, size=22):
    """System + palette + custom colour buttons appended to box.
    on_pick(value) gets "" (follow GNOME) or "#rrggbb"."""
    btns = {}
    first = None
    current = (current or "").lower()

    def add(value, child, tip):
        nonlocal first
        b = Gtk.ToggleButton(child=child, tooltip_text=tip, css_classes=["hbo-swatch"],
                             active=value == current, valign=Gtk.Align.CENTER)
        if first is None:
            first = b
        else:
            b.set_group(first)
        b.connect("toggled", lambda w, v=value: w.get_active() and on_pick(v))
        btns[value] = b
        box.append(b)
        return b

    add("", Gtk.Image(icon_name="emblem-system-symbolic", pixel_size=12), "Follow system")
    for hexv, name in ACCENTS:
        dot = Gtk.Box(css_classes=["hbo-dot"], width_request=size - 6, height_request=size - 6)
        prov = Gtk.CssProvider()
        prov.load_from_string(".hbo-dot { background: %s; border-radius: 99px; }" % hexv)
        dot.get_style_context().add_provider(prov, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        add(hexv, dot, name)
    custom = Gtk.ColorDialogButton(dialog=Gtk.ColorDialog(with_alpha=False), tooltip_text="Custom",
                                   valign=Gtk.Align.CENTER, css_classes=["hbo-custom-accent"])
    rgba = Gdk.RGBA()
    rgba.parse(current if current and current not in btns else "#9141ac")
    custom.set_rgba(rgba)

    def picked(w, _p):
        c = w.get_rgba()
        value = "#%02x%02x%02x" % (round(c.red * 255), round(c.green * 255), round(c.blue * 255))
        for b in btns.values():
            b.set_active(False)
        on_pick(value)
    custom.connect("notify::rgba", picked)
    box.append(custom)
    return btns
