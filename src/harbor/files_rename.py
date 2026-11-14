"""Harbor Files: batch rename (like PowerRename).

plan() is pure: old names + rules -> new names and one error per row.
apply() renames in two steps (old -> temporary -> new) so swaps and chains
like a->b, b->c work. RenameDialog shows a live old -> new preview.
"""

import os
import re
import uuid

from gi.repository import Adw, Gio, GLib, Gtk, Pango

from . import files_core as fc

CASES = ("Keep", "lowercase", "UPPERCASE", "Title Case")


def _case(text, case):
    if case == "lowercase":
        return text.lower()
    if case == "UPPERCASE":
        return text.upper()
    if case == "Title Case":
        return " ".join(w[:1].upper() + w[1:].lower() for w in text.split(" "))
    return text


def plan(names, find="", replace="", regex=False, match_case=False, pattern="",
         start=1, case="Keep", include_ext=False, taken=()):
    """[(new_name, error_or_None)] for each name, in order.

    pattern: "" keeps the (replaced) stem; otherwise "*" is the old stem,
    "#" runs are the number padded to their length (start counts up).
    taken: other names already in the folder (not being renamed).
    """
    out = []
    flags = 0 if match_case else re.IGNORECASE
    rx = None
    err_rx = None
    if find:
        try:
            rx = re.compile(find if regex else re.escape(find), flags)
        except re.error as e:
            err_rx = "Bad pattern: %s" % e
    for i, name in enumerate(names):
        if err_rx:
            out.append((name, err_rx))
            continue
        stem, ext = (name, "") if include_ext else fc.split_ext(name)
        if rx is not None:
            try:
                stem = rx.sub(replace if regex else replace.replace("\\", "\\\\"), stem)
            except (re.error, IndexError) as e:
                out.append((name, "Bad replacement: %s" % e))
                continue
        if pattern:
            num = start + i
            stem = re.sub(r"#+", lambda m: str(num).zfill(len(m.group(0))),
                          pattern.replace("*", "\0")).replace("\0", stem)
        new = _case(stem, case) + ext
        out.append((new, fc.validate_name(new)))
    # duplicates among the new names, or clashes with files that stay
    count = {}
    for new, _e in out:
        count[new] = count.get(new, 0) + 1
    renamed = set(names)
    final = []
    for (new, err), old in zip(out, names):
        if not err and count[new] > 1:
            err = "Two items would get this name"
        elif not err and new != old and new in taken and new not in renamed:
            err = "An item with this name is already here"
        final.append((new, err))
    return final


def apply(folder, pairs):
    """Rename [(old, new)] inside folder. Returns [(new_path, old_name)] done
    (for undo) and raises nothing: a failed item is reported in errors."""
    pairs = [(o, n) for o, n in pairs if o != n]
    temps, done, errors = [], [], []
    for old, new in pairs:
        tmp = ".hb-rename-%s" % uuid.uuid4().hex[:10]
        try:
            Gio.File.new_for_path(os.path.join(folder, old)).set_display_name(tmp, None)
            temps.append((tmp, old, new))
        except GLib.Error as e:
            errors.append("%s: %s" % (old, e.message))
    for tmp, old, new in temps:
        src = Gio.File.new_for_path(os.path.join(folder, tmp))
        try:
            if os.path.lexists(os.path.join(folder, new)):
                raise GLib.Error("an item with this name is already here")
            src.set_display_name(new, None)
            done.append((os.path.join(folder, new), old))
        except GLib.Error as e:
            errors.append("%s: %s" % (old, e.message))
            try:
                src.set_display_name(old, None)
            except GLib.Error:
                pass
    return done, errors


class RenameDialog(Adw.Dialog):
    def __init__(self, win, folder, names, on_done):
        super().__init__(title="Rename %d items" % len(names), content_width=720,
                         content_height=560)
        self.folder, self.names, self.on_done = folder, names, on_done
        try:
            self.taken = set(os.listdir(folder)) - set(names)
        except OSError:
            self.taken = set()
        tv = Adw.ToolbarView()
        head = Adw.HeaderBar(show_end_title_buttons=False, show_start_title_buttons=False)
        cancel = Gtk.Button(label="Cancel")
        cancel.connect("clicked", lambda *_: self.close())
        head.pack_start(cancel)
        self.ok = Gtk.Button(label="Rename", css_classes=["suggested-action"])
        self.ok.connect("clicked", self._apply)
        head.pack_end(self.ok)
        tv.add_top_bar(head)

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10, margin_start=16,
                      margin_end=16, margin_top=8, margin_bottom=16)
        grid = Gtk.Grid(column_spacing=10, row_spacing=8)
        self.find = Gtk.Entry(placeholder_text="Search for", hexpand=True)
        self.repl = Gtk.Entry(placeholder_text="Replace with", hexpand=True)
        self.pattern = Gtk.Entry(placeholder_text="New name, e.g. Holiday ### or * - ##",
                                 hexpand=True,
                                 tooltip_text="* = the old name, ### = 001, 002, 003…")
        self.start = Gtk.SpinButton.new_with_range(0, 999999, 1)
        self.start.set_value(1)
        self.regex = Gtk.CheckButton(label="Regular expression")
        self.mcase = Gtk.CheckButton(label="Match case")
        self.inc_ext = Gtk.CheckButton(label="Include extension")
        self.case = Gtk.DropDown.new_from_strings(list(CASES))
        rows = (("Find", self.find), ("Replace", self.repl), ("Name pattern", self.pattern))
        for r, (lab, w) in enumerate(rows):
            grid.attach(Gtk.Label(label=lab, xalign=0), 0, r, 1, 1)
            grid.attach(w, 1, r, 3, 1)
        grid.attach(Gtk.Label(label="Start at", xalign=0), 0, 3, 1, 1)
        grid.attach(self.start, 1, 3, 1, 1)
        grid.attach(Gtk.Label(label="Case", xalign=1, hexpand=True), 2, 3, 1, 1)
        grid.attach(self.case, 3, 3, 1, 1)
        opts = Gtk.Box(spacing=14)
        for w in (self.regex, self.mcase, self.inc_ext):
            opts.append(w)
        grid.attach(opts, 1, 4, 3, 1)
        box.append(grid)

        self.listbox = Gtk.ListBox(css_classes=["boxed-list"], selection_mode=Gtk.SelectionMode.NONE)
        box.append(Gtk.ScrolledWindow(child=self.listbox, vexpand=True))
        self.summary = Gtk.Label(xalign=0, css_classes=["hb-dim"])
        box.append(self.summary)
        tv.set_content(box)
        self.set_child(tv)
        for w in (self.find, self.repl, self.pattern):
            w.connect("changed", self._update)
        self.start.connect("value-changed", self._update)
        for w in (self.regex, self.mcase, self.inc_ext):
            w.connect("toggled", self._update)
        self.case.connect("notify::selected", self._update)
        self._update()

    def _rules(self):
        return dict(find=self.find.get_text(), replace=self.repl.get_text(),
                    regex=self.regex.get_active(), match_case=self.mcase.get_active(),
                    pattern=self.pattern.get_text(), start=int(self.start.get_value()),
                    case=CASES[self.case.get_selected()], include_ext=self.inc_ext.get_active(),
                    taken=self.taken)

    def _update(self, *_):
        self.result = plan(self.names, **self._rules())
        self.listbox.remove_all()
        changed = bad = 0
        for old, (new, err) in zip(self.names[:500], self.result[:500]):
            row = Gtk.Box(spacing=10, margin_start=10, margin_end=10, margin_top=5,
                          margin_bottom=5)
            row.append(Gtk.Label(label=old, xalign=0, hexpand=True,
                                 ellipsize=Pango.EllipsizeMode.MIDDLE, width_chars=20))
            row.append(Gtk.Image(icon_name="go-next-symbolic"))
            nl = Gtk.Label(label=new, xalign=0, hexpand=True, width_chars=20,
                           ellipsize=Pango.EllipsizeMode.MIDDLE)
            if err:
                nl.add_css_class("error")
                nl.set_tooltip_text(err)
            elif new != old:
                nl.add_css_class("accent")
            row.append(nl)
            self.listbox.append(row)
        for old, (new, err) in zip(self.names, self.result):
            bad += bool(err)
            changed += (not err and new != old)
        self.summary.set_label(
            "%d will be renamed" % changed + (", %d have a problem" % bad if bad else ""))
        self.ok.set_sensitive(changed > 0 and bad == 0)

    def _apply(self, *_):
        pairs = [(o, n) for o, (n, _e) in zip(self.names, self.result)]
        done, errors = apply(self.folder, pairs)
        self.close()
        self.on_done(done, errors)
