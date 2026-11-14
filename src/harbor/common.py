"""Shared look and widgets for the Harbor apps.

Every Harbor app imports this module. It holds the Windows 11-style theme
(light and dark, accent taken from the GNOME accent colour), the window
shell, the left navigation list, setting cards and small helpers.
"""

import os
import shlex
import string
import sys

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, Gio, GLib, GObject, Gtk  # noqa: E402

from . import VERSION  # noqa: E402

# HARBOR_DRY_RUN=1: never run commands that change the system. run_cmd() and
# spawn() only log them to stderr (and to DRY_RUN_LOG for tests).
DRY_RUN = os.environ.get("HARBOR_DRY_RUN") == "1"
DRY_RUN_LOG = []

# ------------------------------------------------------------------ theme

PALETTES = {
    "dark": dict(
        window="#202020", sidebar="#202020", surface="#272727", card="#2b2b2b",
        card_hover="#323232", card_active="#282828", border="rgba(255,255,255,0.07)",
        divider="rgba(255,255,255,0.08)", text="#ffffff", dim="rgba(255,255,255,0.62)",
        faint="rgba(255,255,255,0.40)", nav_hover="rgba(255,255,255,0.06)",
        nav_sel="rgba(255,255,255,0.09)", entry="#2d2d2d", entry_border="rgba(255,255,255,0.10)",
        shadow="rgba(0,0,0,0.35)", heat1="rgba(255,196,0,0.10)", heat2="rgba(255,150,0,0.22)",
        heat3="rgba(255,90,40,0.34)",
    ),
    "light": dict(
        window="#f3f3f3", sidebar="#f3f3f3", surface="#f9f9f9", card="#fbfbfb",
        card_hover="#f6f6f6", card_active="#f0f0f0", border="rgba(0,0,0,0.07)",
        divider="rgba(0,0,0,0.08)", text="#1a1a1a", dim="rgba(0,0,0,0.60)",
        faint="rgba(0,0,0,0.42)", nav_hover="rgba(0,0,0,0.04)",
        nav_sel="rgba(0,0,0,0.06)", entry="#ffffff", entry_border="rgba(0,0,0,0.10)",
        shadow="rgba(0,0,0,0.10)", heat1="rgba(255,210,0,0.16)", heat2="rgba(255,170,0,0.28)",
        heat3="rgba(255,110,40,0.36)",
    ),
    # "System" look (Harbor Files > Options > Style): every colour comes from
    # the libadwaita / system theme, Harbor keeps only its layout.
    "system": dict(
        window="var(--window-bg-color)", sidebar="var(--sidebar-bg-color)",
        surface="var(--view-bg-color)", card="var(--card-bg-color)",
        card_hover="alpha(currentColor,0.05)", card_active="alpha(currentColor,0.08)",
        border="var(--border-color)", divider="var(--border-color)",
        text="var(--window-fg-color)", dim="alpha(currentColor,0.62)",
        faint="alpha(currentColor,0.42)", nav_hover="alpha(currentColor,0.07)",
        nav_sel="alpha(currentColor,0.10)", entry="var(--view-bg-color)",
        entry_border="var(--border-color)", shadow="alpha(black,0.15)",
        heat1="rgba(255,210,0,0.16)", heat2="rgba(255,170,0,0.28)",
        heat3="rgba(255,110,40,0.36)",
    ),
}

CSS = string.Template("""
window.hb-window, window.hb-window > contents { background: $window; color: $text; }
.hb-window headerbar { background: $window; box-shadow: none; border: none; min-height: 40px; }
.hb-window headerbar .title { font-weight: normal; font-size: 12px; }
.hb-sidebar { background: $sidebar; }
.hb-content { background: $window; }
.hb-surface { background: $surface; border-top-left-radius: 8px; border: 1px solid $border; }

/* navigation list (Settings, Task Manager, Files sidebar) */
list.hb-nav, list.hb-nav > row { background: transparent; }
list.hb-nav > row { border-radius: 5px; padding: 0; margin: 1px 6px; min-height: 36px; }
list.hb-nav > row:hover { background: $nav_hover; }
list.hb-nav > row:selected, list.hb-nav > row:selected:hover { background: $nav_sel; color: $text; }
list.hb-nav > row:selected .hb-nav-pill { background: var(--accent-bg-color); }
.hb-nav-pill { min-width: 3px; min-height: 16px; border-radius: 2px; background: transparent; }
.hb-nav-label { font-size: 14px; }
.hb-nav-heading { font-size: 12px; color: $dim; margin: 10px 18px 2px 18px; }

/* icon badge: solid accent square with a white symbolic icon */
.hb-badge { background: var(--accent-bg-color); color: white; border-radius: 6px;
            min-width: 24px; min-height: 24px; }
.hb-badge.hb-badge-big { border-radius: 9px; min-width: 36px; min-height: 36px; }

/* text */
.hb-title { font-size: 28px; font-weight: 600; }
.hb-subtitle { font-size: 20px; font-weight: 600; }
.hb-section { font-size: 14px; font-weight: 600; margin-top: 14px; margin-bottom: 4px; }
.hb-dim { color: $dim; }
.hb-faint { color: $faint; }
.hb-small { font-size: 12px; }
.hb-crumb { font-size: 28px; font-weight: 600; color: $dim; }
.hb-crumb-last { font-size: 28px; font-weight: 600; color: $text; }
.hb-accent-text { color: var(--accent-color); }

/* setting cards */
.hb-card { background: $card; border: 1px solid $border; border-radius: 6px;
           padding: 9px 16px; min-height: 42px; color: $text; }
button.hb-card { font-weight: normal; box-shadow: none; }
button.hb-card:hover { background: $card_hover; }
button.hb-card:active { background: $card_active; }
.hb-card-title { font-size: 14px; }
.hb-card-sub { font-size: 12px; color: $dim; }
.hb-card-group { border-radius: 6px; }
.hb-tile { background: $card; border: 1px solid $border; border-radius: 8px; padding: 12px; }
button.hb-tile:hover { background: $card_hover; }

/* entries and buttons */
entry.hb-search, searchentry.hb-search { background: $entry; border: 1px solid $entry_border;
    border-bottom: 1px solid $faint; border-radius: 5px; min-height: 32px; }
button.hb-accent { background: var(--accent-bg-color); color: var(--accent-fg-color);
    border-radius: 5px; padding: 5px 16px; font-weight: normal; }
button.hb-accent:hover { filter: brightness(1.08); }
button.hb-plain { background: $card; border: 1px solid $border; border-radius: 5px;
    padding: 5px 14px; font-weight: normal; }
button.hb-plain:hover { background: $card_hover; }
button.hb-link { color: var(--accent-color); background: none; padding: 0 2px; font-weight: normal; }
button.hb-tool { background: none; border-radius: 5px; padding: 4px 8px; font-weight: normal; min-height: 28px; }
button.hb-tool:hover { background: $nav_hover; }
button.hb-tool:disabled { opacity: 0.4; }
.hb-divider { background: $divider; min-width: 1px; min-height: 1px; }

/* table (Task Manager, Files details) */
.hb-table-head { font-size: 12px; color: $dim; }
.hb-table-head-big { font-size: 18px; color: $text; font-weight: 600; }
.hb-heat1 { background: $heat1; }
.hb-heat2 { background: $heat2; }
.hb-heat3 { background: $heat3; }
columnview.hb-table, columnview.hb-table > listview { background: transparent; }
columnview.hb-table > listview > row { border-radius: 0; }
columnview.hb-table > listview > row:hover { background: $nav_hover; }
columnview.hb-table > listview > row:selected { background: $nav_sel; color: $text; }
columnview.hb-table > header > button { background: transparent; border: none; box-shadow: none; }
""")


class _Theme:
    def __init__(self):
        self.provider = Gtk.CssProvider()
        self.extra = []
        self.style = "harbor"      # "harbor" (own colours) or "system"
        self.manager = Adw.StyleManager.get_default()
        self.manager.connect("notify::dark", lambda *_: self.reload())

    @property
    def dark(self):
        return self.manager.get_dark()

    def palette(self):
        if self.style == "system":
            return PALETTES["system"]
        return PALETTES["dark" if self.dark else "light"]

    def set_style(self, style):
        style = "system" if style == "system" else "harbor"
        if style != self.style:
            self.style = style
            self.reload()

    def reload(self):
        pal = self.palette()
        css = CSS.substitute(pal)
        for tmpl in self.extra:
            css += tmpl.substitute(pal)
        self.provider.load_from_string(css)


THEME = None


def apply_theme(extra_css=""):
    """Install the Harbor CSS once. extra_css may use $window, $card, ... ."""
    global THEME
    if THEME is None:
        THEME = _Theme()
        display = Gdk.Display.get_default()
        Gtk.StyleContext.add_provider_for_display(
            display, THEME.provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
    if extra_css:
        THEME.extra.append(string.Template(extra_css))
    THEME.reload()
    return THEME


def heat_class(fraction):
    """CSS class for a Task Manager style heat cell (0.0 .. 1.0)."""
    if fraction >= 0.6:
        return "hb-heat3"
    if fraction >= 0.25:
        return "hb-heat2"
    if fraction >= 0.03:
        return "hb-heat1"
    return None


# ------------------------------------------------------------------ window shell

class HarborWindow(Adw.ApplicationWindow):
    """Window with a Windows-style title bar (title left, controls right)."""

    def __init__(self, app, title, width=1100, height=720, icon=None):
        super().__init__(application=app, title=title)
        self.add_css_class("hb-window")
        self.set_default_size(width, height)
        self.toolbar = Adw.ToolbarView()
        self.header = Adw.HeaderBar()
        self.header.set_decoration_layout(":minimize,maximize,close")
        tbox = Gtk.Box(spacing=8)
        if icon:
            img = Gtk.Image.new_from_icon_name(icon)
            img.set_pixel_size(16)
            tbox.append(img)
        self.title_label = Gtk.Label(label=title, css_classes=["hb-small"])
        tbox.append(self.title_label)
        self.header.pack_start(tbox)
        self.header.set_title_widget(Gtk.Box())
        self.toolbar.add_top_bar(self.header)
        self.toast = Adw.ToastOverlay()
        self.toolbar.set_content(self.toast)
        self.set_content(self.toolbar)

    def set_body(self, widget):
        self.toast.set_child(widget)

    def notify_toast(self, text, timeout=3):
        self.toast.add_toast(Adw.Toast(title=text, timeout=timeout))


# ------------------------------------------------------------------ navigation list

class NavList(Gtk.ListBox):
    """Win11 navigation: icon + label, accent pill on the selected row.

    items: list of (key, label, icon_name). Use add_heading() for group titles.
    Emits "selected" (key) when the user picks a row.
    """

    __gsignals__ = {"selected": (GObject.SignalFlags.RUN_FIRST, None, (str,))}

    def __init__(self, items=(), badges=False):
        super().__init__(css_classes=["hb-nav"], selection_mode=Gtk.SelectionMode.SINGLE)
        self.badges = badges
        self.rows = {}
        self._quiet = False
        for key, label, icon in items:
            self.add_item(key, label, icon)
        self.connect("row-selected", self._on_row)

    def add_item(self, key, label, icon, suffix=None):
        box = Gtk.Box(spacing=12, margin_start=4, margin_end=10)
        pill = Gtk.Box(css_classes=["hb-nav-pill"], valign=Gtk.Align.CENTER)
        box.append(pill)
        if self.badges:
            box.append(icon_badge(icon))
        else:
            img = Gtk.Image.new_from_icon_name(icon)
            img.set_pixel_size(16)
            box.append(img)
        lab = Gtk.Label(label=label, xalign=0, hexpand=True, css_classes=["hb-nav-label"],
                        ellipsize=3)
        box.append(lab)
        if suffix is not None:
            box.append(suffix)
        row = Gtk.ListBoxRow(child=box)
        row.key = key
        row.label = lab
        self.append(row)
        self.rows[key] = row
        return row

    def add_heading(self, text):
        row = Gtk.ListBoxRow(selectable=False, activatable=False)
        row.set_child(Gtk.Label(label=text, xalign=0, css_classes=["hb-nav-heading"]))
        row.key = None
        self.append(row)
        return row

    def select(self, key, emit=False):
        row = self.rows.get(key)
        if row is None:
            self.unselect_all()
            return
        self._quiet = not emit
        self.select_row(row)
        self._quiet = False

    def _on_row(self, _lb, row):
        if row is not None and row.key and not self._quiet:
            self.emit("selected", row.key)


# ------------------------------------------------------------------ small widgets

def icon_badge(icon_name, big=False, color=None):
    """Accent-coloured rounded square with a white symbolic icon."""
    # homogeneous centres the icon without hexpand (hexpand would spread
    # to every parent box and stretch the row)
    box = Gtk.Box(css_classes=["hb-badge"] + (["hb-badge-big"] if big else []),
                  halign=Gtk.Align.CENTER, valign=Gtk.Align.CENTER, homogeneous=True)
    img = Gtk.Image.new_from_icon_name(icon_name)
    img.set_pixel_size(20 if big else 14)
    box.append(img)
    if color:
        prov = Gtk.CssProvider()
        prov.load_from_string(".hb-badge { background: %s; }" % color)
        box.get_style_context().add_provider(prov, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION + 1)
    return box


def label(text, *classes, xalign=0, wrap=False, **kw):
    lab = Gtk.Label(label=text, xalign=xalign, css_classes=list(classes), **kw)
    if wrap:
        lab.set_wrap(True)
        lab.set_wrap_mode(2)  # WORD_CHAR
    return lab


def title(text):
    return label(text, "hb-title")


def section(text):
    return label(text, "hb-section")


class Card(Gtk.Box):
    """A Windows settings row: [icon] title / subtitle ........ [suffix] [>].

    on_activate makes the whole card a button with a chevron (unless
    chevron=False). suffix is any widget (Gtk.Switch, Gtk.DropDown, button).
    """

    def __init__(self, title, subtitle=None, icon=None, suffix=None, on_activate=None,
                 chevron=None, badge=False):
        super().__init__()
        inner = Gtk.Box(spacing=14)
        if icon:
            if badge:
                inner.append(icon_badge(icon))
            else:
                img = Gtk.Image.new_from_icon_name(icon)
                img.set_pixel_size(18)
                inner.append(img)
        texts = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, hexpand=True,
                        valign=Gtk.Align.CENTER)
        self.title_label = label(title, "hb-card-title", ellipsize=3)
        texts.append(self.title_label)
        self.sub_label = label(subtitle or "", "hb-card-sub", ellipsize=3)
        self.sub_label.set_visible(bool(subtitle))
        texts.append(self.sub_label)
        inner.append(texts)
        self.suffix_box = Gtk.Box(spacing=10, valign=Gtk.Align.CENTER)
        if suffix is not None:
            self.suffix_box.append(suffix)
        inner.append(self.suffix_box)
        if chevron is None:
            chevron = on_activate is not None and suffix is None
        if chevron:
            chev = Gtk.Image.new_from_icon_name("go-next-symbolic")
            chev.add_css_class("hb-dim")
            inner.append(chev)
        if on_activate is not None:
            btn = Gtk.Button(child=inner, css_classes=["hb-card"], hexpand=True)
            btn.connect("clicked", lambda *_: on_activate())
            self.append(btn)
        else:
            inner.add_css_class("hb-card")
            inner.set_hexpand(True)
            self.append(inner)

    def set_subtitle(self, text):
        self.sub_label.set_label(text or "")
        self.sub_label.set_visible(bool(text))

    def set_title(self, text):
        self.title_label.set_label(text)


def card_group(*cards, spacing=3):
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=spacing,
                  css_classes=["hb-card-group"])
    for c in cards:
        box.append(c)
    return box


def switch(active=False, on_toggle=None):
    """Gtk.Switch with an On/Off label in front, like Windows."""
    box = Gtk.Box(spacing=10, valign=Gtk.Align.CENTER)
    lab = label("On" if active else "Off", "hb-dim")
    sw = Gtk.Switch(active=active, valign=Gtk.Align.CENTER)

    def changed(s, _p):
        lab.set_label("On" if s.get_active() else "Off")
        if on_toggle and not getattr(sw, "_quiet", False):
            on_toggle(s.get_active())
    sw.connect("notify::active", changed)
    box.append(lab)
    box.append(sw)
    box.switch = sw

    def set_quiet(value):
        sw._quiet = True
        sw.set_active(value)
        sw._quiet = False
    box.set_quiet = set_quiet
    return box


def dropdown(options, selected=0, on_change=None):
    dd = Gtk.DropDown.new_from_strings(list(options))
    dd.set_selected(selected)
    dd.set_valign(Gtk.Align.CENTER)
    if on_change:
        dd.connect("notify::selected", lambda d, _p: on_change(d.get_selected()))
    return dd


def scrolled(child, hscroll=False):
    sw = Gtk.ScrolledWindow(vexpand=True, hexpand=True, child=child)
    sw.set_policy(Gtk.PolicyType.AUTOMATIC if hscroll else Gtk.PolicyType.NEVER,
                  Gtk.PolicyType.AUTOMATIC)
    return sw


def page_box(max_width=1000, margin=36):
    """Centered, width-limited vertical page body inside a scroller."""
    clamp = Adw.Clamp(maximum_size=max_width, tightening_threshold=max_width)
    body = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4,
                   margin_start=margin, margin_end=margin, margin_top=18, margin_bottom=28)
    clamp.set_child(body)
    return scrolled(clamp), body


# ------------------------------------------------------------------ helpers

def human_size(n, decimals=1):
    n = float(n or 0)
    for unit in ("B", "KB", "MB", "GB", "TB"):
        if abs(n) < 1024 or unit == "TB":
            if unit == "B":
                return "%d B" % n
            return ("%." + str(decimals) + "f %s") % (n, unit)
        n /= 1024.0
    return "%d B" % n


def human_rate(n):
    return human_size(n) + "/s"


def run_cmd(argv, callback=None, stdin=None, mutating=True):
    """Run argv asynchronously; callback(ok, stdout, stderr).

    Commands that change the system must keep mutating=True so that
    HARBOR_DRY_RUN=1 skips them. Read-only queries pass mutating=False.
    """
    if DRY_RUN and mutating:
        DRY_RUN_LOG.append(list(argv))
        print("[dry-run]", shlex.join(argv), file=sys.stderr)
        if callback:
            GLib.idle_add(lambda: (callback(True, "", ""), False)[1])
        return None
    flags = Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE
    if stdin is not None:
        flags |= Gio.SubprocessFlags.STDIN_PIPE
    try:
        proc = Gio.Subprocess.new(argv, flags)
    except GLib.Error as e:
        if callback:
            GLib.idle_add(lambda: (callback(False, "", e.message), False)[1])
        return None

    def done(p, res):
        try:
            _ok, out, err = p.communicate_utf8_finish(res)
            ok = p.get_successful()
        except GLib.Error as e:
            ok, out, err = False, "", e.message
        if callback:
            callback(ok, out or "", err or "")
    proc.communicate_utf8_async(stdin, None, done)
    return proc


def run_sync(argv, timeout=5):
    """Read-only query, returns stdout or "" on any failure."""
    import subprocess
    try:
        return subprocess.run(argv, capture_output=True, text=True, timeout=timeout).stdout
    except (OSError, subprocess.SubprocessError):
        return ""


def spawn(argv):
    """Start a program and forget it (honours HARBOR_DRY_RUN)."""
    if DRY_RUN:
        DRY_RUN_LOG.append(list(argv))
        print("[dry-run] spawn", shlex.join(argv), file=sys.stderr)
        return True
    try:
        GLib.spawn_async(argv, flags=GLib.SpawnFlags.SEARCH_PATH)
        return True
    except GLib.Error:
        return False


def have(program):
    return GLib.find_program_in_path(program) is not None


def greeting(hour=None):
    if hour is None:
        hour = GLib.DateTime.new_now_local().get_hour()
    if hour < 5:
        return "Good evening"
    if hour < 12:
        return "Good morning"
    if hour < 18:
        return "Good afternoon"
    return "Good evening"


def user_display_name():
    name = GLib.get_real_name()
    if not name or name == "Unknown":
        name = GLib.get_user_name()
    return name


def about_dialog(parent, app_name, icon, comment):
    dlg = Adw.AboutDialog(application_name=app_name, application_icon=icon,
                          version=VERSION, developer_name="Md. Hafijur Rahman",
                          comments=comment, license_type=Gtk.License.MIT_X11)
    dlg.present(parent)
