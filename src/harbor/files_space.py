"""Harbor Files: "What's using space". Lists what is inside a folder,
biggest first, with a bar for each. Double-click a folder to look inside it;
Back goes up again; "Show in window" opens the folder in the current tab."""

import os
import threading

from gi.repository import Adw, GLib, Gtk, Pango

from . import files_core as fc
from .common import human_size


def scan(folder, cancelled=lambda: False, budget=20.0):
    """[(name, path, is_dir, size, complete)] for folder's children, biggest
    first. Shares one time budget across the whole folder."""
    import time
    out = []
    end = time.monotonic() + budget
    try:
        entries = list(os.scandir(folder))
    except OSError:
        return out
    for e in entries:
        if cancelled():
            return []
        try:
            if e.is_dir(follow_symlinks=False):
                left = max(0.5, end - time.monotonic())
                size, complete = fc.dir_size(e.path, cancelled, budget=left)
                if size is None:
                    continue
                out.append((e.name, e.path, True, size, complete))
            else:
                out.append((e.name, e.path, False, e.stat(follow_symlinks=False).st_size, True))
        except OSError:
            continue
    out.sort(key=lambda r: r[3], reverse=True)
    return out


class SpaceDialog(Adw.Dialog):
    def __init__(self, win, folder):
        super().__init__(title="What's using space", content_width=640, content_height=600)
        self.win, self.root, self.folder = win, folder, folder
        self.gen = 0
        tv = Adw.ToolbarView()
        head = Adw.HeaderBar()
        self.back = Gtk.Button(icon_name="go-up-symbolic", tooltip_text="Up one folder")
        self.back.connect("clicked", lambda *_: self.load(os.path.dirname(self.folder)))
        head.pack_start(self.back)
        self.title = Adw.WindowTitle(title="What's using space")
        head.set_title_widget(self.title)
        show = Gtk.Button(label="Show in window")
        show.connect("clicked", self._show)
        head.pack_end(show)
        tv.add_top_bar(head)
        self.stack = Gtk.Stack()
        self.stack.add_named(Adw.Spinner(), "busy")
        self.listbox = Gtk.ListBox(css_classes=["boxed-list"], margin_start=14, margin_end=14,
                                   margin_top=10, margin_bottom=14,
                                   selection_mode=Gtk.SelectionMode.NONE, valign=Gtk.Align.START)
        self.listbox.connect("row-activated", self._row)
        self.stack.add_named(Gtk.ScrolledWindow(child=self.listbox, vexpand=True), "list")
        tv.set_content(self.stack)
        self.set_child(tv)
        self.connect("closed", lambda *_: setattr(self, "gen", self.gen + 1))
        self.load(folder)

    def load(self, folder):
        self.gen += 1
        gen = self.gen
        self.folder = folder
        self.back.set_sensitive(folder != "/")
        self.title.set_subtitle(folder)
        self.stack.set_visible_child_name("busy")

        def work():
            rows = scan(folder, lambda: gen != self.gen)
            GLib.idle_add(self._fill, gen, rows)
        threading.Thread(target=work, daemon=True, name="hb-space").start()

    def _fill(self, gen, rows):
        if gen != self.gen:
            return False
        self.listbox.remove_all()
        total = sum(r[3] for r in rows) or 1
        partial = any(not r[4] for r in rows)
        self.title.set_title("%s in %s%s" % (human_size(total), os.path.basename(self.folder) or "/",
                                             "+" if partial else ""))
        if not rows:
            self.listbox.append(Gtk.Label(label="Nothing here", margin_top=20, margin_bottom=20,
                                          css_classes=["hb-dim"]))
        for name, path, is_dir, size, complete in rows[:400]:
            row = Gtk.ListBoxRow(activatable=is_dir)
            row.path = path if is_dir else None
            box = Gtk.Box(spacing=10, margin_start=10, margin_end=10, margin_top=6,
                          margin_bottom=6)
            box.append(Gtk.Image(icon_name="folder-symbolic" if is_dir else "text-x-generic-symbolic"))
            col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4, hexpand=True)
            col.append(Gtk.Label(label=name, xalign=0, ellipsize=Pango.EllipsizeMode.MIDDLE))
            col.append(Gtk.ProgressBar(fraction=size / total))
            box.append(col)
            pct = 100.0 * size / total
            box.append(Gtk.Label(label="%s%s  %4.1f%%" % (human_size(size), "" if complete else "+", pct),
                                 xalign=1, width_chars=16, css_classes=["numeric", "hb-dim"]))
            box.append(Gtk.Image(icon_name="go-next-symbolic", opacity=1 if is_dir else 0))
            row.set_child(box)
            self.listbox.append(row)
        self.stack.set_visible_child_name("list")
        return False

    def _row(self, _lb, row):
        if getattr(row, "path", None):
            self.load(row.path)

    def _show(self, *_):
        folder = self.folder
        self.close()
        self.win.open_location(folder)


def space_usage(win):
    tab = win.tab()
    folder = tab.folder_path() if tab is not None else None
    sel = tab.view.selected_items() if tab is not None and tab.kind() == "folder" else []
    if len(sel) == 1 and sel[0].is_dir and sel[0].path and not sel[0].remote:
        folder = sel[0].path
    if not folder or not os.path.isdir(folder):
        folder = os.path.expanduser("~")
    SpaceDialog(win, folder).present(win)
