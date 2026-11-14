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
    ("icons", "Icons", "Modern uses Harbor's own icon set",
     (("modern", "Modern"), ("system", "System icon theme"))),
    ("folder_color", "Folder colour", "Every folder icon in the Modern set",
     (("classic", "Classic yellow"), ("accent", "Follow accent colour"))
     + tuple((k, PALETTE_LABELS[k]) for k in fc.FOLDER_PALETTE) + (("custom", "Custom"),)),
)
STARTUP_CHOICES = (
    ("open_to", "Open Harbor Files to", "What a new window shows first",
     (("thispc", "This PC"), ("home", "Home"), ("last", "Last folder"))),
)
LAYOUT_CHOICES = (
    ("command_bar", "Command bar", None,
     (("win11", "Windows 11"), ("ribbon", "Windows 10 ribbon"))),
    ("menu_style", "Right-click menu", "Show more options (Shift+F10) always opens the classic one",
     (("win11", "Windows 11"), ("classic", "Classic (Windows 10)"))),
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
    "open_archives_in_harbor": ("Open archives in Harbor",
                                "Double-click lists a .zip / .7z / .tar here instead of the "
                                "default app"),
    "image_thumbnails": ("Image thumbnails", None),
    "doc_thumbnails": ("Document thumbnails", "Office pages in Large icons (slower)"),
    "office_preview": ("Office preview", None),
    "remote_thumbnails": ("Thumbnails on network locations", "Downloads files to make them"),
}


def _all_choices():
    return APPEARANCE_CHOICES + STARTUP_CHOICES + LAYOUT_CHOICES + BEHAVIOUR_CHOICES


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

    # ------------------------------------------------------------ pages
    def _page(self, name, title, icon):
        p = Adw.PreferencesPage(name=name, title=title, icon_name=icon)
        self.add(p)
        return p

    def _build_appearance(self):
        p = self._page("appearance", "Appearance", "applications-graphics-symbolic")
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
        p.add(g)

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

    def sync(self, key, value):
        """Reflect a change made elsewhere (menu, ribbon, keyboard)."""
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


OptionsDialog = SettingsDialog
