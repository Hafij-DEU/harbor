"""Harbor Files: the Options dialog (… > Options, or File > Options in the ribbon).

Every row writes fc.Settings through FilesWindow.apply_setting(), which also
applies the change live to every open window.
"""

from gi.repository import Adw, Gtk

from . import common
from . import files_core as fc

# (settings key, title, subtitle, [(value, label)])
GENERAL_CHOICES = (
    ("open_to", "Open Harbor Files to", "What a new window shows first",
     (("thispc", "This PC"), ("home", "Home"))),
    ("menu_style", "Right-click menu", "Show more options (Shift+F10) always opens the classic one",
     (("win11", "Windows 11"), ("classic", "Classic (Windows 10)"))),
    ("command_bar", "Command bar", None,
     (("win11", "Windows 11"), ("ribbon", "Windows 10 ribbon"))),
)
VIEW_CHOICES = (
    ("style", "Style", "System uses your GNOME colours and accent",
     (("win11", "Windows 11"), ("system", "System"))),
    ("icons", "Icons", "Modern uses Harbor's own icon set",
     (("modern", "Modern"), ("system", "System icon theme"))),
)
GENERAL_SWITCHES = (
    ("show_recent", "Show recent files in Home", None),
)
VIEW_SWITCHES = (
    ("show_extensions", "Show file name extensions", None),
    ("show_hidden", "Show hidden items", None),
)


def _check_choices():
    """Every choice row matches fc.Settings.CHOICES (used by the tests)."""
    for key, _t, _s, opts in GENERAL_CHOICES + VIEW_CHOICES:
        assert tuple(v for v, _l in opts) == fc.Settings.CHOICES[key], key
    return True


class OptionsDialog(Adw.PreferencesDialog):
    def __init__(self, win):
        super().__init__(title="Options", search_enabled=False)
        self.win = win
        self.rows = {}
        general = Adw.PreferencesPage(name="general", title="General", icon_name="preferences-system-symbolic")
        g1 = Adw.PreferencesGroup(title="Browse")
        for spec in GENERAL_CHOICES:
            g1.add(self._combo(*spec))
        for spec in GENERAL_SWITCHES:
            g1.add(self._switch(*spec))
        general.add(g1)
        g2 = Adw.PreferencesGroup(title="Default file manager",
                                  description="Folders opened from other apps use the default "
                                              "file manager.")
        self.fm_row = Adw.ActionRow(title="Harbor Files", subtitle="Checking…")
        self.fm_btn = Gtk.Button(label="Make default", valign=Gtk.Align.CENTER)
        self.fm_btn.connect("clicked", self._fm_clicked)
        self.fm_row.add_suffix(self.fm_btn)
        g2.add(self.fm_row)
        general.add(g2)
        self.add(general)

        view = Adw.PreferencesPage(name="view", title="View", icon_name="view-grid-symbolic")
        g3 = Adw.PreferencesGroup(title="Look")
        for spec in VIEW_CHOICES:
            g3.add(self._combo(*spec))
        view.add(g3)
        g4 = Adw.PreferencesGroup(title="Files and folders")
        for spec in VIEW_SWITCHES:
            g4.add(self._switch(*spec))
        view.add(g4)
        self.add(view)
        self._fm_state = "unknown"
        self.refresh_fm()

    # ------------------------------------------------------------ rows
    def _combo(self, key, title, subtitle, opts):
        values = [v for v, _l in opts]
        row = Adw.ComboRow(title=title, model=Gtk.StringList.new([lab for _v, lab in opts]))
        if subtitle:
            row.set_subtitle(subtitle)
        cur = self.win.settings.get(key)
        row.set_selected(values.index(cur) if cur in values else 0)
        row.connect("notify::selected",
                    lambda r, _p: self.win.apply_setting(key, values[r.get_selected()]))
        self.rows[key] = row
        return row

    def _switch(self, key, title, subtitle):
        row = Adw.SwitchRow(title=title, active=bool(self.win.settings.get(key)))
        if subtitle:
            row.set_subtitle(subtitle)
        row.connect("notify::active", lambda r, _p: self.win.apply_setting(key, r.get_active()))
        self.rows[key] = row
        return row

    def sync(self, key, value):
        """Reflect a change made elsewhere (menu, ribbon, keyboard)."""
        row = self.rows.get(key)
        if isinstance(row, Adw.SwitchRow) and row.get_active() != bool(value):
            row.set_active(bool(value))

    # ------------------------------------------------------------ default file manager
    def refresh_fm(self):
        common.run_cmd(fc.default_fm_query_argv(), self._fm_got, mutating=False)

    def _fm_got(self, ok, out, _err):
        state = fc.default_fm_state(out if ok else "")
        self._fm_state = state
        current = (out or "").strip().splitlines()[0] if (ok and (out or "").strip()) else ""
        if state == "harbor":
            self.fm_row.set_subtitle("Harbor Files is the default file manager")
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
