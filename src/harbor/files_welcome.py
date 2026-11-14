"""Harbor Files: first-run welcome, feature tour and setup wizard.

Shown once, the first time a window opens (setting "onboarded" is False).
Closing it at any step never blocks the file manager: whatever was not
confirmed stays as it was. Theme and accent preview live while choosing and
are put back if the wizard is closed early; everything else is applied on
"Start using Harbor Files". Settings > Appearance > Welcome reopens it.
"""

from gi.repository import Adw, GLib, Gtk, Pango

from .files_options import ACCENTS, DESIGNS, accent_swatches, design_thumb

APP_NAME = "Harbor Files"
STEPS = (("welcome", "Welcome"), ("tour", "Tour"), ("design", "Design"),
         ("colours", "Colours"), ("layout", "Layout"), ("finish", "Finish"))

# (icon, gradient class, title, text, chips)
FEATURES = (
    ("folder-open-symbolic", "hbw-g1", "Fast, familiar file management",
     "Copy, move, rename and undo with the shortcuts you already know. Batch rename "
     "with a live preview, and open archives like folders.",
     ("Undo", "Batch rename", "Archives")),
    ("view-dual-symbolic", "hbw-g2", "Tabs and split view",
     "Keep many folders open in tabs and put two folders side by side to drag "
     "files between them.",
     ("Tabs", "Split view", "Drag and drop")),
    ("drive-harddisk-symbolic", "hbw-g3", "See what fills your drives",
     "The drive button in the status bar shows used and free space, and What's using space "
     "finds the biggest folders in seconds.",
     ("Storage overview", "Folder sizes", "Space analyzer")),
    ("view-reveal-symbolic", "hbw-g4", "Preview without opening",
     "Press Space for Quick Look, or keep the preview pane open for images, video, "
     "PDFs, text and Office documents.",
     ("Quick Look", "Preview pane", "Thumbnails")),
    ("system-search-symbolic", "hbw-g5", "Search and reach everything",
     "Search the current folder as you type, open network shares, and send files "
     "to your phone over Wi-Fi.",
     ("Search", "Network", "Phone")),
    ("applications-graphics-symbolic", "hbw-g6", "Make it yours",
     "Pick the Windows 10 or Windows 11 layout, an accent colour, folder colours, "
     "light or dark, compact or comfortable.",
     ("Designs", "Accent colour", "Folder colours")),
)

THEMES = (("follow", "System default"), ("light", "Light"), ("dark", "Dark"))
DENSITIES = (("comfortable", "Comfortable", "Roomy rows, easy to click"),
             ("compact", "Compact", "More rows on screen"))
VIEWS = (("large", "Icons", "Big thumbnails in a grid"),
         ("medium", "Small icons", "More items in the grid"),
         ("details", "Details", "Name, date, type and size columns"))

CSS = """
.hbw { background: var(--window-bg-color); }
.hbw-steps { padding: 6px 0 2px 0; }
.hbw-step { font-size: 12px; padding: 3px 10px; border-radius: 99px; color: alpha(currentColor, 0.55); }
.hbw-step.done { color: alpha(currentColor, 0.85); }
.hbw-step.current { background: alpha(var(--accent-bg-color), 0.16); color: var(--accent-color);
    font-weight: 700; }
progressbar.hbw-progress trough { min-height: 3px; border-radius: 0; background: alpha(currentColor, 0.06); }
progressbar.hbw-progress progress { min-height: 3px; border-radius: 0; }
.hbw-hero-icon { animation: hbw-pop 700ms cubic-bezier(0.2, 0.9, 0.3, 1.25) both; }
.hbw-hero-title { font-size: 30px; font-weight: 800; animation: hbw-rise 600ms 120ms ease-out both; }
.hbw-hero-sub { font-size: 15px; animation: hbw-rise 600ms 220ms ease-out both; }
.hbw-hero-btns { animation: hbw-rise 600ms 320ms ease-out both; }
.hbw-title { font-size: 22px; font-weight: 800; }
.hbw-sub { font-size: 14px; }
@keyframes hbw-pop { from { opacity: 0; transform: scale(0.6); } to { opacity: 1; transform: none; } }
@keyframes hbw-rise { from { opacity: 0; transform: translateY(14px); } to { opacity: 1; transform: none; } }
@keyframes hbw-float { 0% { transform: translateY(0); } 50% { transform: translateY(-6px); }
    100% { transform: translateY(0); } }
.hbw-feature-tile { border-radius: 28px; min-width: 132px; min-height: 132px;
    box-shadow: 0 10px 30px alpha(black, 0.18); animation: hbw-float 4s ease-in-out infinite; }
.hbw-feature-tile image { color: white; -gtk-icon-size: 64px; }
.hbw-g1 { background: linear-gradient(135deg, #4f8cff, #2b5fd9); }
.hbw-g2 { background: linear-gradient(135deg, #8b5cf6, #6d28d9); }
.hbw-g3 { background: linear-gradient(135deg, #f59e0b, #ea580c); }
.hbw-g4 { background: linear-gradient(135deg, #ec4899, #be185d); }
.hbw-g5 { background: linear-gradient(135deg, #14b8a6, #0f766e); }
.hbw-g6 { background: linear-gradient(135deg, #22c55e, #15803d); }
.hbw-chip { font-size: 12px; padding: 3px 10px; border-radius: 99px; background: alpha(currentColor, 0.08); }
button.hbw-card { padding: 10px; border-radius: 14px; background: alpha(currentColor, 0.04);
    transition: all 200ms ease; }
button.hbw-card:hover { background: alpha(currentColor, 0.08); }
button.hbw-card:checked { background: alpha(var(--accent-bg-color), 0.12);
    box-shadow: inset 0 0 0 2px var(--accent-bg-color); }
.hbw-check { color: var(--accent-bg-color); opacity: 0; transition: opacity 200ms; }
button.hbw-card:checked .hbw-check { opacity: 1; }
.hbw-th { border-radius: 8px; border: 1px solid alpha(black, 0.15); }
.hbw-th-light { background: #f3f3f3; }
.hbw-th-dark { background: #202020; }
.hbw-th-system { background: linear-gradient(110deg, #f3f3f3 50%, #202020 50%); }
.hbw-th-bar { min-height: 9px; margin: 6px 8px 4px 8px; border-radius: 4px; }
.hbw-th-light .hbw-th-bar, .hbw-th-light .hbw-th-line { background: #dcdcdc; }
.hbw-th-dark .hbw-th-bar, .hbw-th-dark .hbw-th-line { background: #3a3a3a; }
.hbw-th-system .hbw-th-bar, .hbw-th-system .hbw-th-line { background: alpha(#808080, 0.55); }
.hbw-th-line { min-height: 5px; margin: 3px 8px; border-radius: 3px; }
.hbw-th-accent { min-height: 5px; min-width: 28px; margin: 3px 8px; border-radius: 3px;
    background: var(--accent-bg-color); }
.hbw-mini { border-radius: 8px; background: alpha(currentColor, 0.06); padding: 6px; }
.hbw-mini-row { background: alpha(currentColor, 0.18); border-radius: 2px; min-height: 4px; }
.hbw-mini-sq { background: alpha(var(--accent-bg-color), 0.55); border-radius: 3px; }
.hbw-bar { padding: 10px 18px; border-top: 1px solid alpha(currentColor, 0.08); }
button.hbw-next { padding: 6px 22px; border-radius: 99px; }
"""


def _lab(text, *cls, wrap=False, xalign=0.5):
    lab = Gtk.Label(label=text, css_classes=list(cls), wrap=wrap, xalign=xalign,
                    justify=Gtk.Justification.CENTER if xalign == 0.5 else Gtk.Justification.LEFT)
    if wrap:
        lab.set_wrap_mode(Pango.WrapMode.WORD_CHAR)
    return lab


def _card_group(options, current, on_pick, make_preview, columns=3):
    """Big toggle cards: [(value, title, subtitle)] -> (box, {value: button})."""
    box = Gtk.FlowBox(homogeneous=True, max_children_per_line=columns,
                      min_children_per_line=min(columns, 2), column_spacing=14, row_spacing=14,
                      selection_mode=Gtk.SelectionMode.NONE, halign=Gtk.Align.CENTER)
    btns, first = {}, None
    for value, title, sub in options:
        inner = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        prev = make_preview(value)
        prev.set_halign(Gtk.Align.CENTER)
        inner.append(prev)
        head = Gtk.Box(spacing=6, halign=Gtk.Align.CENTER)
        head.append(Gtk.Image(icon_name="emblem-ok-symbolic", css_classes=["hbw-check"]))
        head.append(_lab(title, "heading"))
        inner.append(head)
        if sub:
            sl = _lab(sub, "dim-label", "caption", wrap=True)
            sl.set_max_width_chars(26)
            inner.append(sl)
        b = Gtk.ToggleButton(child=inner, css_classes=["hbw-card"], active=value == current)
        if first is None:
            first = b
        else:
            b.set_group(first)
        b.connect("toggled", lambda w, v=value: w.get_active() and on_pick(v))
        btns[value] = b
        box.append(b)
    return box, btns


def theme_thumb(value):
    t = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, css_classes=["hbw-th", "hbw-th-" + (
        "system" if value == "follow" else value)], width_request=150, height_request=84)
    t.append(Gtk.Box(css_classes=["hbw-th-bar"]))
    for _ in range(3):
        t.append(Gtk.Box(css_classes=["hbw-th-line"]))
    t.append(Gtk.Box(css_classes=["hbw-th-accent"], halign=Gtk.Align.START))
    return t


def density_thumb(value):
    t = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, css_classes=["hbw-mini"],
                spacing=8 if value == "comfortable" else 3, width_request=150, height_request=84)
    for _ in range(5 if value == "comfortable" else 9):
        t.append(Gtk.Box(css_classes=["hbw-mini-row"]))
    return t


def view_thumb(value):
    t = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, css_classes=["hbw-mini"], spacing=5,
                width_request=150, height_request=84)
    if value == "details":
        for _ in range(7):
            r = Gtk.Box(spacing=6)
            r.append(Gtk.Box(css_classes=["hbw-mini-sq"], width_request=6, height_request=5))
            r.append(Gtk.Box(css_classes=["hbw-mini-row"], hexpand=True))
            r.append(Gtk.Box(css_classes=["hbw-mini-row"], width_request=22))
            r.append(Gtk.Box(css_classes=["hbw-mini-row"], width_request=16))
            t.append(r)
        return t
    n, s = (4, 26) if value == "large" else (6, 15)
    for _ in range(2 if value == "large" else 3):
        r = Gtk.Box(spacing=6, homogeneous=True)
        for _ in range(n):
            r.append(Gtk.Box(css_classes=["hbw-mini-sq"], height_request=s))
        t.append(r)
    return t


class WelcomeDialog(Adw.Dialog):
    def __init__(self, win):
        super().__init__(content_width=860, content_height=640, title="Welcome")
        self.win, self.s = win, win.settings
        self.add_css_class("hbw")
        prov = Gtk.CssProvider()
        prov.load_from_string(CSS)
        Gtk.StyleContext.add_provider_for_display(self.get_display() or win.get_display(), prov,
                                                  Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION + 1)
        self._css = prov
        self.finished = False
        self.orig = {k: self.s.get(k) for k in ("theme", "accent")}
        view = self.s.get("view")
        self.choice = {
            "ui_design": self.s.get("ui_design"),
            "theme": self.s.get("theme"),
            "accent": self.s.get("accent") or "",
            "density": self.s.get("density"),
            "view": "details" if view == "details" else self.s.get("icon_size"),
            "preview_pane": bool(self.s.get("preview_pane")),
            "nav_pane": bool(self.s.get("nav_pane")),
            "show_quick_access": bool(self.s.get("show_quick_access")),
            "show_storage": bool(self.s.get("show_storage")),
            "folder_sizes": bool(self.s.get("folder_sizes")),
        }
        self.auto_id = 0

        tv = Adw.ToolbarView(top_bar_style=Adw.ToolbarStyle.FLAT)
        head = Adw.HeaderBar(show_title=False)
        steps = Gtk.Box(spacing=4, css_classes=["hbw-steps"])
        self.step_labels = {}
        for key, name in STEPS:
            lab = Gtk.Label(label=name, css_classes=["hbw-step"])
            self.step_labels[key] = lab
            steps.append(lab)
        head.set_title_widget(steps)
        head.set_show_title(True)
        tv.add_top_bar(head)
        self.progress = Gtk.ProgressBar(css_classes=["hbw-progress"])
        tv.add_top_bar(self.progress)

        self.stack = Gtk.Stack(transition_type=Gtk.StackTransitionType.SLIDE_LEFT_RIGHT,
                               transition_duration=380, vexpand=True)
        self.stack.add_named(self._page_welcome(), "welcome")
        self.stack.add_named(self._page_tour(), "tour")
        self.stack.add_named(self._page_design(), "design")
        self.stack.add_named(self._page_colours(), "colours")
        self.stack.add_named(self._page_layout(), "layout")
        self.finish_page = Gtk.Box()
        self.stack.add_named(self.finish_page, "finish")
        tv.set_content(self.stack)

        bar = Gtk.CenterBox(css_classes=["hbw-bar"])
        self.back = Gtk.Button(label="Back")
        self.back.connect("clicked", lambda *_: self.go(-1))
        bar.set_start_widget(self.back)
        self.counter = Gtk.Label(css_classes=["dim-label", "caption"])
        bar.set_center_widget(self.counter)
        ends = Gtk.Box(spacing=8)
        self.skip = Gtk.Button(label="Skip tour", css_classes=["flat"])
        self.skip.connect("clicked", lambda *_: self.show_step("design"))
        ends.append(self.skip)
        self.next = Gtk.Button(label="Next", css_classes=["suggested-action", "hbw-next"])
        self.next.connect("clicked", lambda *_: self.go(1))
        ends.append(self.next)
        bar.set_end_widget(ends)
        tv.add_bottom_bar(bar)
        self.bottom = bar
        self.set_child(tv)
        self.connect("closed", self._closed)
        self.show_step("welcome")

    # ------------------------------------------------------------- pages
    def _scroller(self, child):
        clamp = Adw.Clamp(maximum_size=760, child=child, margin_start=24, margin_end=24)
        return Gtk.ScrolledWindow(child=clamp, hscrollbar_policy=Gtk.PolicyType.NEVER,
                                  vexpand=True)

    def _header(self, title, sub):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6, margin_top=18,
                      margin_bottom=18)
        box.append(_lab(title, "hbw-title"))
        box.append(_lab(sub, "hbw-sub", "dim-label", wrap=True))
        return box

    def _page_welcome(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=14, valign=Gtk.Align.CENTER,
                      halign=Gtk.Align.CENTER, margin_start=40, margin_end=40)
        icon = Gtk.Image(icon_name=self.win.get_icon_name() or "system-file-manager",
                         pixel_size=128, css_classes=["hbw-hero-icon"])
        box.append(icon)
        box.append(_lab("Welcome to %s" % APP_NAME, "hbw-hero-title"))
        sub = _lab("A fast, familiar file manager for your Linux desktop. Take a short tour, "
                   "then make it look and feel the way you like.", "hbw-hero-sub", "dim-label",
                   wrap=True)
        sub.set_max_width_chars(52)
        box.append(sub)
        btns = Gtk.Box(spacing=10, halign=Gtk.Align.CENTER, margin_top=14,
                       css_classes=["hbw-hero-btns"])
        tour = Gtk.Button(label="Take the tour", css_classes=["suggested-action", "pill"])
        tour.connect("clicked", lambda *_: self.show_step("tour"))
        setup = Gtk.Button(label="Skip to setup", css_classes=["pill"])
        setup.connect("clicked", lambda *_: self.show_step("design"))
        btns.append(tour)
        btns.append(setup)
        box.append(btns)
        later = Gtk.Button(label="Start with the current settings", css_classes=["flat"],
                           halign=Gtk.Align.CENTER)
        later.connect("clicked", lambda *_: self._skip_all())
        box.append(later)
        return box

    def _page_tour(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        self.carousel = Adw.Carousel(vexpand=True, allow_scroll_wheel=True, spacing=24,
                                     reveal_duration=500)
        for icon, grad, title, text, chips in FEATURES:
            page = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=14,
                           valign=Gtk.Align.CENTER, halign=Gtk.Align.CENTER, hexpand=True,
                           margin_start=40, margin_end=40)
            tile = Gtk.Box(css_classes=["hbw-feature-tile", grad], halign=Gtk.Align.CENTER,
                           margin_bottom=12)
            tile.append(Gtk.Image(icon_name=icon, hexpand=True, vexpand=True))
            page.append(tile)
            page.append(_lab(title, "hbw-title"))
            body = _lab(text, "hbw-sub", "dim-label", wrap=True)
            body.set_max_width_chars(54)
            page.append(body)
            row = Gtk.Box(spacing=6, halign=Gtk.Align.CENTER, margin_top=4)
            for c in chips:
                row.append(_lab(c, "hbw-chip"))
            page.append(row)
            self.carousel.append(page)
        self.carousel.connect("page-changed", lambda *_: self._sync_buttons())
        box.append(self.carousel)
        dots = Adw.CarouselIndicatorDots(carousel=self.carousel, margin_bottom=6)
        box.append(dots)
        # any touch of the carousel stops the automatic slides
        for ctl in (Gtk.GestureClick(), Gtk.EventControllerScroll(
                flags=Gtk.EventControllerScrollFlags.BOTH_AXES)):
            ctl.connect("pressed" if isinstance(ctl, Gtk.GestureClick) else "scroll",
                        lambda *_: self._stop_auto() or False)
            self.carousel.add_controller(ctl)
        return box

    def _page_design(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        box.append(self._header("Choose your interface",
                                "Each design changes the whole window layout, not only colours."))
        subs = {v: s for v, _t, s in DESIGNS}
        opts = [(v, t if v != "win12" else "Windows 12 concept", subs[v]) for v, t, _s in DESIGNS]
        cards, self.design_btns = _card_group(
            opts, self.choice["ui_design"], lambda v: self._pick("ui_design", v),
            lambda v: design_thumb(v, 200, 118))
        box.append(cards)
        note = _lab("The Windows 12 concept is Harbor's own experimental design. It is not an "
                    "official Windows theme.", "dim-label", "caption", wrap=True)
        note.set_margin_top(14)
        box.append(note)
        return self._scroller(box)

    def _page_colours(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        box.append(self._header("Pick your colours",
                                "Changes show right away so you can see them."))
        cards, self.theme_btns = _card_group(
            [(v, t, None) for v, t in THEMES], self.choice["theme"],
            lambda v: self._pick("theme", v, live=True), theme_thumb)
        box.append(cards)
        box.append(_lab("Accent colour", "heading", xalign=0.5))
        sw = Gtk.Box(spacing=6, halign=Gtk.Align.CENTER, margin_top=10, margin_bottom=10)
        self.accent_btns = accent_swatches(self.choice["accent"],
                                           lambda v: self._pick("accent", v, live=True), sw, 30)
        box.append(sw)
        box.append(_lab("The first button follows your GNOME accent colour; the last one picks "
                        "any colour.", "dim-label", "caption", wrap=True))
        box.set_margin_bottom(12)
        b = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=14)
        b.append(box)
        return self._scroller(b)

    def _page_layout(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, margin_bottom=16)
        box.append(self._header("Set up your layout", "How folders look when you open them."))
        dens, self.density_btns = _card_group(DENSITIES, self.choice["density"],
                                              lambda v: self._pick("density", v), density_thumb,
                                              columns=2)
        box.append(dens)
        views, self.view_btns = _card_group(VIEWS, self.choice["view"],
                                            lambda v: self._pick("view", v), view_thumb)
        views.set_margin_top(10)
        box.append(_lab("Folder view", "heading", xalign=0.5))
        box.append(views)
        g = Adw.PreferencesGroup(title="Panels and sidebar", margin_top=14)
        self.switches = {}
        for key, title, sub in (
                ("preview_pane", "Preview panel", "Show the selected file on the right"),
                ("nav_pane", "Sidebar", "Folders, drives and network on the left"),
                ("show_quick_access", "Quick access in the sidebar", "Your pinned folders"),
                ("show_storage", "Storage overview button", "Drive button by the view buttons"),
                ("folder_sizes", "Folder sizes", "Work out folder sizes in Details view")):
            row = Adw.SwitchRow(title=title, subtitle=sub, active=self.choice[key])
            row.connect("notify::active", lambda r, _p, k=key: self._pick(k, r.get_active()))
            self.switches[key] = row
            g.add(row)
        box.append(g)
        return self._scroller(box)

    def _build_finish(self):
        child = self.finish_page.get_first_child()
        if child is not None:
            self.finish_page.remove(child)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, margin_bottom=16)
        box.append(self._header("You're all set", "Check your choices. Change any of them now, "
                                "or later in Settings > Appearance."))
        g = Adw.PreferencesGroup()
        names = {v: t for v, t, _s in DESIGNS}
        names["win12"] = "Windows 12 concept"
        accent = self.choice["accent"]
        accent_name = dict(ACCENTS).get(accent, accent.upper() or "") if accent else "Follow system"
        on_off = lambda k: "On" if self.choice[k] else "Off"  # noqa: E731
        sidebar = "Off" if not self.choice["nav_pane"] else (
            "With Quick access" if self.choice["show_quick_access"] else "On")
        rows = (("Interface", names.get(self.choice["ui_design"], ""), "design"),
                ("Theme", dict(THEMES)[self.choice["theme"]], "colours"),
                ("Accent colour", accent_name, "colours"),
                ("Layout density", dict((v, t) for v, t, _s in DENSITIES)[self.choice["density"]],
                 "layout"),
                ("Folder view", dict((v, t) for v, t, _s in VIEWS)[self.choice["view"]], "layout"),
                ("Preview panel", on_off("preview_pane"), "layout"),
                ("Sidebar", sidebar, "layout"),
                ("Storage overview", on_off("show_storage"), "layout"))
        for title, value, step in rows:
            row = Adw.ActionRow(title=title, subtitle=value)
            ch = Gtk.Button(label="Change", valign=Gtk.Align.CENTER, css_classes=["flat"])
            ch.connect("clicked", lambda *_, s=step: self.show_step(s))
            row.add_suffix(ch)
            g.add(row)
        box.append(g)
        start = Gtk.Button(label="Start using %s" % APP_NAME, halign=Gtk.Align.CENTER,
                           css_classes=["suggested-action", "pill"], margin_top=18)
        start.connect("clicked", lambda *_: self.finish())
        box.append(start)
        self.finish_page.append(self._scroller(box))

    # -------------------------------------------------------- navigation
    def order(self):
        return [k for k, _n in STEPS]

    def current(self):
        return self.stack.get_visible_child_name()

    def show_step(self, key):
        if key == "finish":
            self._build_finish()
        self.stack.set_visible_child_name(key)
        if key == "tour":
            self.carousel.scroll_to(self.carousel.get_nth_page(0), False)
            self._start_auto()
        else:
            self._stop_auto()
        self._sync_buttons()

    def go(self, d):
        cur = self.current()
        if cur == "tour":
            pos = round(self.carousel.get_position())
            n = self.carousel.get_n_pages()
            if 0 <= pos + d < n:
                self._stop_auto()
                self.carousel.scroll_to(self.carousel.get_nth_page(pos + d), True)
                return
        if cur == "finish" and d > 0:
            self.finish()
            return
        steps = self.order()
        i = max(0, min(len(steps) - 1, steps.index(cur) + d))
        self.show_step(steps[i])

    def _sync_buttons(self):
        cur = self.current()
        steps = self.order()
        i = steps.index(cur)
        for k, lab in self.step_labels.items():
            j = steps.index(k)
            for c in ("current", "done"):
                lab.remove_css_class(c)
            lab.add_css_class("current" if j == i else "done" if j < i else "x")
        frac = i / (len(steps) - 1)
        if cur == "tour":
            n = self.carousel.get_n_pages()
            frac = (i + self.carousel.get_position() / max(1, n)) / (len(steps) - 1)
        self.progress.set_fraction(frac)
        self.bottom.set_visible(cur != "welcome")
        self.back.set_sensitive(i > 0)
        self.skip.set_visible(cur == "tour")
        if cur == "tour":
            pos = round(self.carousel.get_position())
            self.counter.set_label("Feature %d of %d" % (pos + 1, self.carousel.get_n_pages()))
            self.next.set_label("Next" if pos + 1 < self.carousel.get_n_pages() else "Set it up")
        else:
            self.counter.set_label("Step %d of %d" % (i, len(steps) - 1) if i else "")
            self.next.set_label("Start using %s" % APP_NAME if cur == "finish"
                                else "Review" if cur == "layout" else "Next")

    def _start_auto(self):
        self._stop_auto()
        self.auto_id = GLib.timeout_add(5000, self._auto_step)

    def _stop_auto(self):
        if self.auto_id:
            GLib.source_remove(self.auto_id)
            self.auto_id = 0

    def _auto_step(self):
        if self.current() != "tour":
            self.auto_id = 0
            return False
        pos = round(self.carousel.get_position())
        if pos + 1 >= self.carousel.get_n_pages():
            self.auto_id = 0
            return False
        self.carousel.scroll_to(self.carousel.get_nth_page(pos + 1), True)
        return True

    # ------------------------------------------------------------ choices
    def _pick(self, key, value, live=False):
        self.choice[key] = value
        if live:
            self.win.apply_setting(key, value)

    def _skip_all(self):
        self.finished = True     # keep everything exactly as it is
        self.s.set("onboarded", True)
        self.close()

    def finish(self):
        c = self.choice
        w = self.win
        for key in ("ui_design", "theme", "accent", "density", "show_quick_access",
                    "show_storage", "folder_sizes"):
            w.apply_setting(key, c[key])
        act = w.lookup_action("view")
        if act is not None:
            act.activate(GLib.Variant("s", c["view"]))
        for key, action in (("preview_pane", "preview-pane"), ("nav_pane", "nav-pane")):
            a = w.lookup_action(action)
            if a is not None and bool(a.get_state()) != c[key]:
                a.change_state(GLib.Variant("b", c[key]))
            else:
                w.settings.set(key, c[key])
        self.finished = True
        self.s.set("onboarded", True)
        self.close()

    def _closed(self, *_):
        self._stop_auto()
        if not self.finished:
            # closed early: put back the live previews, never ask again
            for k, v in self.orig.items():
                if self.s.get(k) != v:
                    self.win.apply_setting(k, v)
            self.s.set("onboarded", True)
        Gtk.StyleContext.remove_provider_for_display(self.win.get_display(), self._css)


def show(win):
    dlg = WelcomeDialog(win)
    dlg.present(win)
    return dlg
