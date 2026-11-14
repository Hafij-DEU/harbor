"""Harbor Files: Quick Look. Space shows a big preview of the selected file;
Left/Right (or Up/Down) step through the folder and move the selection with
it; Space or Escape closes."""

from gi.repository import Adw, Gdk, Gtk

from .files_preview import PreviewPane


class QuickLook(Adw.Dialog):
    def __init__(self, win, tab, item):
        super().__init__(content_width=max(640, int((win.get_width() or 1200) * 0.8)),
                         content_height=max(480, int((win.get_height() or 800) * 0.85)))
        self.win, self.tab = win, tab
        self.items = [i for i in tab.view.items()]
        self.index = max(0, self.items.index(item)) if item in self.items else 0
        tv = Adw.ToolbarView()
        head = Adw.HeaderBar()
        self.title = Adw.WindowTitle()
        head.set_title_widget(self.title)
        prev = Gtk.Button(icon_name="go-previous-symbolic", tooltip_text="Previous (Left)")
        nxt = Gtk.Button(icon_name="go-next-symbolic", tooltip_text="Next (Right)")
        prev.connect("clicked", lambda *_: self.step(-1))
        nxt.connect("clicked", lambda *_: self.step(1))
        head.pack_start(prev)
        head.pack_start(nxt)
        opn = Gtk.Button(label="Open", tooltip_text="Open with the default app (Enter)")
        opn.connect("clicked", lambda *_: self.open_current())
        head.pack_end(opn)
        tv.add_top_bar(head)
        self.pane = PreviewPane(win, quick=True)
        tv.set_content(self.pane)
        self.set_child(tv)
        keys = Gtk.EventControllerKey(propagation_phase=Gtk.PropagationPhase.CAPTURE)
        keys.connect("key-pressed", self._on_key)
        self.add_controller(keys)
        self.connect("closed", lambda *_: self.pane._clear())
        self.show_current()

    def current(self):
        return self.items[self.index] if self.items else None

    def show_current(self):
        it = self.current()
        if it is None:
            self.close()
            return
        self.title.set_title(it.name)
        self.title.set_subtitle("%d of %d" % (self.index + 1, len(self.items)))
        self.pane.show_item(it)

    def step(self, d):
        if not self.items:
            return
        self.index = (self.index + d) % len(self.items)
        it = self.current()
        self.tab.view.select_item(it)
        pos = self.tab.view.position_of(it)
        if pos >= 0:
            self.tab.view.scroll_to(pos)
        self.show_current()

    def open_current(self):
        it = self.current()
        self.close()
        if it is not None:
            self.win.open_items(self.tab, [it])

    def _on_key(self, _c, keyval, _code, state):
        if state & (Gdk.ModifierType.CONTROL_MASK | Gdk.ModifierType.ALT_MASK):
            return False
        if keyval in (Gdk.KEY_space, Gdk.KEY_Escape):
            self.close()
            return True
        if keyval in (Gdk.KEY_Right, Gdk.KEY_Down):
            self.step(1)
            return True
        if keyval in (Gdk.KEY_Left, Gdk.KEY_Up):
            self.step(-1)
            return True
        if keyval in (Gdk.KEY_Return, Gdk.KEY_KP_Enter):
            self.open_current()
            return True
        return False


def quick_look(win):
    tab = win.tab()
    if tab is None or tab.kind() in ("home", "thispc"):
        return False
    sel = tab.view.selected_items()
    if not sel:
        items = tab.view.items()
        if not items:
            return False
        sel = items[:1]
    QuickLook(win, tab, sel[0]).present(win)
    return True
