"""Memory Sweep page and the Memory optimization settings of Task Manager.

The logic lives in taskmgr_sweep (ported from Memory Sweep); this module only
shows it in the Windows 11 look of the other Task Manager pages. Scans run
in worker threads; every widget change happens on the main loop.
"""

import os
import threading
import time
import types

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, GLib, Gtk, Pango  # noqa: E402

from . import taskmgr_sweep as ms  # noqa: E402
from .common import card_group, dropdown, label, page_box, run_cmd, section, switch  # noqa: E402
from .taskmgr_ui import (Page, bar, confirm, scroll_to_widget, set_bar, spin,  # noqa: E402
                         tool_button)

FILES_SHOWN = 10  # rows before "Show all"
TOP_APPS = 5


def plural(n, word, many=None):
    return "%d %s" % (n, word if n == 1 else (many or word + "s"))


def row(icon, title, subtitle="", wrap=False):
    """A Windows settings row (like common.Card) whose texts may wrap and
    whose suffix box takes several widgets. Returns the row; .title, .sub,
    .suffix, .icon are its parts."""
    box = Gtk.Box(spacing=14, css_classes=["hb-card"])
    img = Gtk.Image(pixel_size=18, valign=Gtk.Align.CENTER)
    if icon:
        img.set_from_icon_name(icon)
    box.append(img)
    texts = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, hexpand=True,
                    valign=Gtk.Align.CENTER)
    t = label(title, "hb-card-title")
    s = label(subtitle, "hb-card-sub")
    for lab in (t, s):
        if wrap:
            lab.set_wrap(True)
            lab.set_wrap_mode(Pango.WrapMode.WORD_CHAR)
        else:
            lab.set_ellipsize(Pango.EllipsizeMode.END)
    s.set_visible(bool(subtitle))
    texts.append(t)
    texts.append(s)
    box.append(texts)
    suffix = Gtk.Box(spacing=10, valign=Gtk.Align.CENTER)
    box.append(suffix)
    box.title, box.sub, box.suffix, box.icon, box.texts = t, s, suffix, img, texts
    return box


def set_sub(r, text):
    r.sub.set_label(text or "")
    r.sub.set_visible(bool(text))


def button(text, callback, accent=False, icon=None):
    if icon:
        child = Gtk.Box(spacing=8)
        img = Gtk.Image.new_from_icon_name(icon)
        img.set_pixel_size(16)
        child.append(img)
        child.append(Gtk.Label(label=text))
        b = Gtk.Button(child=child)
    else:
        b = Gtk.Button(label=text)
    b.add_css_class("hb-accent" if accent else "hb-plain")
    b.set_valign(Gtk.Align.CENTER)
    b.connect("clicked", lambda *_: callback())
    return b


def clear(box):
    while (c := box.get_first_child()) is not None:
        box.remove(c)


# ------------------------------------------------------------------ Memory Sweep page

class SweepPage(Page):
    key = "sweep"
    title = "Memory Sweep"

    def __init__(self, win):
        super().__init__(win)
        self.keep = frozenset(ms.self_and_ancestors())
        self.snap = None
        self.items = None
        self.scanning = False
        self.files_scanning = False
        self.last_scan = 0.0
        self.last_files = 0.0
        self.busy = set()  # ids of instances being ended
        self.show_all_files = False
        self._icons = {}
        self._tips_sig = None
        self._top_sig = None
        self._files_sig = None
        a = self.actions
        a.append(tool_button("view-refresh-symbolic", "Refresh",
                             lambda: self.refresh(files=True)))
        a.append(tool_button("emblem-system-symbolic", "Memory optimization",
                             lambda: self.win.show_page("settings:optimization")))
        self.scroller, body = page_box(max_width=980, margin=20)
        body.set_margin_top(2)
        self.body = body
        body.append(label("See what fills your memory, then end it or remove it. Unsaved "
                          "work in an app you end may be lost.", "hb-tm-sweep-intro", wrap=True))
        self._build_summary(body)
        self.tips_head = section("Suggestions")
        body.append(self.tips_head)
        self.tips_box = card_group()
        body.append(self.tips_box)
        body.append(section("Apps using the most memory"))
        self.top_box = card_group()
        body.append(self.top_box)
        self.top_note = label("", "hb-tm-note", margin_top=4)
        body.append(self.top_note)
        self._build_files(body)
        self.box.append(self.scroller)
        self._loading()

    # ---------------------------------------------------------- layout
    def _build_summary(self, body):
        hero = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8,
                       css_classes=["hb-tm-sweep-hero"], margin_top=6)
        top = Gtk.Box(spacing=10)
        left = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2, hexpand=True)
        left.append(label("Memory in use", "hb-tm-sweep-tile-title"))
        line = Gtk.Box(spacing=8)
        self.ram_used = label("—", "hb-tm-sweep-big")
        line.append(self.ram_used)
        self.ram_of = label("", "hb-tm-sweep-of", valign=Gtk.Align.BASELINE_FILL)
        self.ram_of.set_valign(Gtk.Align.END)
        self.ram_of.set_margin_bottom(4)
        line.append(self.ram_of)
        left.append(line)
        top.append(left)
        right = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2,
                        valign=Gtk.Align.END, halign=Gtk.Align.END)
        self.health = label("", "hb-card-title", xalign=1)
        right.append(self.health)
        self.ram_avail = label("", "hb-card-sub", xalign=1)
        right.append(self.ram_avail)
        top.append(right)
        hero.append(top)
        self.ram_bar = bar()
        hero.append(self.ram_bar)
        body.append(hero)
        tiles = Gtk.Box(spacing=8, homogeneous=True, margin_top=8)
        self.tiles = {}
        for key, icon, title in (("zram", "drive-harddisk-solidstate-symbolic",
                                  "Compressed memory (zram)"),
                                 ("swap", "drive-harddisk-symbolic", "Swap on disk"),
                                 ("files", "folder-symbolic", "Files in memory")):
            t = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6,
                        css_classes=["hb-tm-sweep-tile"])
            head = Gtk.Box(spacing=8)
            img = Gtk.Image.new_from_icon_name(icon)
            img.set_pixel_size(16)
            img.add_css_class("hb-dim")
            head.append(img)
            head.append(label(title, "hb-tm-sweep-tile-title", ellipsize=Pango.EllipsizeMode.END,
                              hexpand=True))
            t.append(head)
            val = label("—", "hb-tm-sweep-tile-value", ellipsize=Pango.EllipsizeMode.END)
            t.append(val)
            pb = bar(thin=True)
            t.append(pb)
            sub = label("", "hb-card-sub", ellipsize=Pango.EllipsizeMode.END)
            t.append(sub)
            tiles.append(t)
            self.tiles[key] = (val, pb, sub)
        body.append(tiles)

    def _build_files(self, body):
        self.files_anchor = Gtk.Box(spacing=10, margin_top=14)
        h = section("Files in memory")
        h.set_margin_top(0)
        h.set_hexpand(True)
        h.set_valign(Gtk.Align.END)
        self.files_anchor.append(h)
        self.remove_all_btn = button("Remove all unused", self.confirm_remove_unused,
                                     icon="edit-clear-all-symbolic")
        self.remove_all_btn.set_margin_bottom(4)
        self.files_anchor.append(self.remove_all_btn)
        body.append(self.files_anchor)
        self.files_info = label("", "hb-tm-sweep-intro", wrap=True)
        body.append(self.files_info)
        self.files_box = card_group()
        body.append(self.files_box)
        foot = Gtk.Box(spacing=12, margin_top=4)
        self.files_note = label("", "hb-tm-note", hexpand=True, valign=Gtk.Align.CENTER)
        foot.append(self.files_note)
        self.show_all_btn = Gtk.Button(label="Show all", css_classes=["hb-link"])
        self.show_all_btn.connect("clicked", lambda *_: self._toggle_files())
        foot.append(self.show_all_btn)
        body.append(foot)

    def _loading(self):
        for box in (self.tips_box, self.top_box, self.files_box):
            box.append(row("content-loading-symbolic", "Looking at memory…"))

    # ---------------------------------------------------------- page hooks
    def on_show(self, sub=None):
        self.refresh(files=True)
        target = {"files": self.files_anchor, "suggestions": self.tips_head}.get(sub or "")
        if target is not None:
            scroll_to_widget(self.scroller, target)

    def on_tick(self):
        now = time.monotonic()
        if now - self.last_scan >= ms.REFRESH_SECONDS:
            self.refresh(files=now - self.last_files >= 30)

    # ---------------------------------------------------------- scanning
    def _sampled(self):
        """Task Manager's latest process readings, when they are fresh."""
        s = self.app.proc_sampler
        if s.last is None or time.monotonic() - s.last > 10 or not s.procs:
            return None, None
        copy = {pid: types.SimpleNamespace(pid=p.pid, ppid=p.ppid, uid=p.uid, comm=p.comm,
                                           exe=p.exe, argv=list(p.argv), start=p.start,
                                           kernel=p.kernel)
                for pid, p in s.procs.items()}
        mem = dict(self.app.sys.mem) if self.app.sys.mem else None
        return copy, mem

    def refresh(self, files=False):
        now = time.monotonic()
        if files and not self.files_scanning:
            self.files_scanning = True
            self.last_files = now

            def work_files():
                try:
                    items = ms.list_items()
                except OSError:
                    items = []
                GLib.idle_add(self._apply_files, items)
            threading.Thread(target=work_files, daemon=True).start()
        if self.scanning:
            return
        self.scanning = True
        self.last_scan = now
        sampled, mem = self._sampled()
        btime = self.app.sys.extra.get("btime") or 0

        def work():
            try:
                procs = ms.procs_from_sampler(sampled, btime or ms.boot_time()) \
                    if sampled else None
                snap = ms.Snapshot(self.keep, procs=procs, mem=mem)
            except Exception as e:  # keep the page alive on odd /proc races
                print("memory sweep scan failed:", e, file=__import__("sys").stderr)
                snap = None
            GLib.idle_add(self._apply, snap)
        threading.Thread(target=work, daemon=True).start()

    def _apply(self, snap):
        self.scanning = False
        if snap is None:
            return False
        self.snap = snap
        self._apply_summary(snap)
        self._apply_tips(snap)
        self._apply_top(snap)
        return False

    # ---------------------------------------------------------- summary
    def _apply_summary(self, snap):
        tot = snap.total or 1
        pct = 100 * snap.used / tot
        self.ram_used.set_label(ms.fmt_size(snap.used))
        self.ram_of.set_label("of %s  ·  %d%%" % (ms.fmt_size(snap.total), round(pct)))
        self.ram_avail.set_label("%s available" % ms.fmt_size(snap.available))
        text, cls = (("Almost full", "hb-tm-warn-text") if pct >= 90 else
                     ("Busy", "hb-tm-busy-text") if pct >= 75 else
                     ("Running smoothly", "hb-tm-ok"))
        self.health.set_label("●  " + text)
        self.health.set_css_classes(["hb-card-title", cls])
        set_bar(self.ram_bar, snap.used / tot, busy_at=0.75)
        val, pb, sub = self.tiles["zram"]
        size, used = snap.swap_parts(zram=True)
        stored = sum(s for _, s, _ in snap.zram)
        in_ram = sum(r for _, _, r in snap.zram)
        if size:
            val.set_label("%s of %s" % (ms.fmt_size(used), ms.fmt_size(size)))
            set_bar(pb, used / size)
            sub.set_label("%.1f× packed, %s of RAM" % (stored / in_ram, ms.fmt_size(in_ram))
                          if in_ram else "Nothing packed yet")
        else:
            val.set_label("Off")
            set_bar(pb, 0)
            sub.set_label("Turn it on in Settings")
        val, pb, sub = self.tiles["swap"]
        size, used = snap.swap_parts(zram=False)
        if size:
            devs = [os.path.basename(d) for d, _, _, _ in snap.swaps if "/zram" not in d]
            val.set_label("%s of %s" % (ms.fmt_size(used), ms.fmt_size(size)))
            set_bar(pb, used / size)
            sub.set_label(", ".join(devs))
        else:
            val.set_label("Off")
            set_bar(pb, 0)
            sub.set_label("No swap file or partition")
        val, pb, sub = self.tiles["files"]
        used = sum(u for _, u, _ in snap.mounts)
        size = sum(t for _, _, t in snap.mounts)
        if size:
            val.set_label(ms.fmt_size(used))
            set_bar(pb, used / size)
            sub.set_label("  ·  ".join("%s %s" % (mp, ms.fmt_size(u))
                                            for mp, u, _ in snap.mounts))
        else:
            val.set_label("—")
            set_bar(pb, 0)
            sub.set_label("No RAM folders")

    # ---------------------------------------------------------- suggestions
    def _apply_tips(self, snap):
        tips = snap.tips()
        sig = [(k, t, key, key in self.busy) for k, t, _p, key in tips]
        if sig == self._tips_sig:
            return
        self._tips_sig = sig
        self.tips_head.set_label("Suggestions (%d)" % len(tips) if tips else "Suggestions")
        clear(self.tips_box)
        if not tips:
            self.tips_box.append(row("object-select-symbolic", "Nothing stands out right now",
                                     "Memory use looks healthy."))
        for kind, text, _page, key in tips:
            self.tips_box.append(self._tip_row(snap, kind, text, key))

    def _tip_row(self, snap, kind, text, key):
        icon = {"files": "folder-symbolic", "end": "process-stop-symbolic",
                "extra": "edit-copy-symbolic", "zram": "drive-harddisk-solidstate-symbolic"
                }.get(kind, "view-grid-symbolic")
        g = snap.group(key) if key else None
        sub = {"files": "Removing files you no longer need frees that memory at once",
               "zram": "Set it up in Settings › Memory optimization"}.get(kind, "")
        if g is not None:
            if kind == "extra":
                sub = "%s. %s keeps running" % (g.extra_note, g.title)
            else:
                sub = g.locked or g.hint or plural(len(g.procs), "process", "processes")
        r = row(icon, text, sub, wrap=True)
        busy = bool(g) and any(i.id in self.busy for i in g.instances + g.extras)
        if kind == "files":
            r.suffix.append(button("Clean up", self.confirm_remove_unused))
        elif kind == "zram":
            r.suffix.append(button("Set up", lambda: self.win.show_page(
                "settings:optimization")))
        elif g is not None:
            if kind == "apps":
                r.suffix.append(button("Review", lambda: self.review(g)))
            if kind == "extra":
                b = button("End extra copy", lambda: self.confirm_extra(g))
            else:
                b = button("End", lambda: self.confirm_end(g, g.instances))
            b.set_sensitive(not busy and (kind == "extra" or not g.locked))
            if busy:
                b.set_label("Ending…")
            if kind == "extra" or not g.locked:
                r.suffix.append(b)
        return r

    def review(self, g):
        """Show the app on the Processes page."""
        q = g.key.split(":", 1)[1] if ":" in g.key else g.key
        self.win.show_page("processes")
        self.win.search.set_text(q)

    # ---------------------------------------------------------- top apps
    def _icon(self, g):
        if g.key in self._icons:
            return self._icons[g.key]
        icon = None
        resolve = getattr(self.app.models.lookup, "resolve", None) if self.app.models else None
        if resolve is not None:
            exes = [i.root.exe for i in g.instances[:3] if i.root.exe]
            try:
                found = resolve({"id": g.key, "desktop": [g.key + ".desktop"],
                                 "kind": "scope"}, exes)
            except Exception:
                found = None
            icon = found[2] if found else None
        self._icons[g.key] = icon
        return icon

    def _apply_top(self, snap):
        groups = snap.groups[:TOP_APPS]
        sig = [(g.key, g.mem >> 20, len(g.procs), g.locked, any(
            i.id in self.busy for i in g.instances)) for g in groups]
        if sig == self._top_sig:
            return
        self._top_sig = sig
        clear(self.top_box)
        biggest = groups[0].mem if groups and groups[0].mem else 1
        for g in groups:
            n, ni = len(g.procs), len(g.instances)
            parts = [plural(n, "process", "processes")]
            if ni > 1:
                parts.append(plural(ni, "instance"))
            if g.swap >= 10 << 20:
                parts.append("%s in swap" % ms.fmt_size(g.swap))
            extras = [e for e in g.extras if not ms.instance_locked(e, self.keep)]
            if extras:
                parts.append("extra copy uses %s" % ms.fmt_size(sum(e.mem for e in extras)))
            parts.append(g.locked or g.hint)
            r = row("application-x-executable-symbolic", g.title,
                    "  ·  ".join(p for p in parts if p))
            icon = self._icon(g)
            if icon is not None:
                r.icon.set_from_gicon(icon)
            pb = bar(thin=True)
            set_bar(pb, g.mem / biggest, warn_at=2)  # relative size, not a warning
            pb.set_size_request(110, -1)
            r.suffix.append(pb)
            r.suffix.append(label(ms.fmt_size(g.mem), "hb-tm-sweep-size", xalign=1,
                                  width_chars=8))
            busy = any(i.id in self.busy for i in g.instances)
            if g.locked:
                lock = Gtk.Image.new_from_icon_name("changes-prevent-symbolic")
                lock.add_css_class("hb-dim")
                lock.set_tooltip_text(g.locked)
                lock.set_size_request(72, -1)
                r.suffix.append(lock)
            else:
                b = button("Ending…" if busy else ("End all" if ni > 1 else "End"),
                           lambda g=g: self.confirm_end(g, g.instances))
                b.set_size_request(72, -1)
                b.set_sensitive(not busy)
                r.suffix.append(b)
            self.top_box.append(r)
        small = sum(1 for g in snap.groups if g.mem < ms.SMALL_GROUP)
        self.top_note.set_label("%s under %s use the rest. Processes lists every app." % (
            plural(len(snap.groups) - len(groups), "more app"), ms.fmt_size(ms.SMALL_GROUP))
            if small else "")

    # ---------------------------------------------------------- ending
    def confirm_extra(self, group, extras=None):
        """End the extra copies an app runs as helpers (see extra_copies), not the app."""
        extras = [e for e in (group.extras if extras is None else extras)
                  if not ms.instance_locked(e, self.keep) and e.id not in self.busy]
        if not extras:
            return
        mem = sum(e.mem for e in extras)
        swap = sum(e.swap for e in extras)
        procs = sum(len(e.tree) for e in extras)
        pids = ", ".join(str(e.root.pid) for e in extras)
        what = (f"the extra copy of {group.title}" if len(extras) == 1
                else f"{len(extras)} extra copies of {group.title}")
        in_swap = f", {ms.fmt_size(swap)} of it in swap" if swap >= 10 << 20 else ""
        body = (f"{group.extra_note} (PID {pids}) is a second {group.title} with its own GPU, "
                f"network and zygote processes: {plural(procs, 'process', 'processes')} using "
                f"about {ms.fmt_size(mem)}{in_swap}. {group.title} itself keeps running.\n\n"
                f"{group.title} may start the copy again after a few seconds. "
                f"The lasting fix belongs in {group.title} itself.")
        confirm(self.win, f"End {what}?", body, "End copy",
                lambda: self.end(what, extras, mem, note=f"{group.title} may start it again"))

    def confirm_end(self, group, instances):
        instances = [i for i in instances
                     if not ms.instance_locked(i, self.keep) and i.id not in self.busy]
        if not instances:
            return
        mem = sum(i.mem for i in instances)
        procs = sum(len(i.tree) for i in instances)
        whole = len(instances) == len(group.instances)
        what = group.title if whole else f"{group.title} (PID {instances[0].root.pid})" \
            if len(instances) == 1 else f"{len(instances)} instances of {group.title}"
        pids = ", ".join(str(i.root.pid) for i in instances[:6])
        if len(instances) > 6:
            pids += ", …"
        body = (f"{plural(procs, 'process', 'processes')} of {group.title} (PID {pids}) using "
                f"about {ms.fmt_size(mem)} will be asked to quit. Unsaved work in them may "
                "be lost.")
        confirm(self.win, f"End {what}?", body, "End", lambda: self.end(what, instances, mem))

    def end(self, what, instances, mem, force=False, note=None):
        for i in instances:
            self.busy.add(i.id)
        self._tips_sig = self._top_sig = None
        if self.snap:
            self._apply_tips(self.snap)
            self._apply_top(self.snap)
        keep = self.keep

        def work():
            left = ms.stop(instances, force=force, keep=keep)
            GLib.idle_add(done, left)

        def done(left):
            for i in instances:
                self.busy.discard(i.id)
            self._tips_sig = self._top_sig = None
            if left:
                t = Adw.Toast(title=f"{what} is still running ({len(left)} processes)",
                              timeout=10)
                t.set_button_label("Force quit")
                t.connect("button-clicked", lambda *_: self.end(what, instances, mem, True, note))
            else:
                t = Adw.Toast(title=f"Ended {what}, about {ms.fmt_size(mem)} freed"
                              + (f". {note}" if note else ""), timeout=8 if note else 5)
            self.win.toast.add_toast(t)
            self.app.refresh_now()
            self.refresh()
            return False
        threading.Thread(target=work, daemon=True).start()

    # ---------------------------------------------------------- files
    def _toggle_files(self):
        self.show_all_files = not self.show_all_files
        self._files_sig = None
        if self.items is not None:
            self._apply_files(self.items, rescan=False)

    def _apply_files(self, items, rescan=True):
        if rescan:
            self.files_scanning = False
        self.items = items
        mounts = self.snap.mounts if self.snap else ms.ram_mounts()
        total = sum(i.size for i in items)
        mine = sum(i.size for i in items if i.own)
        text = (f"Files in {' and '.join(ms.FILE_ROOTS)} live in RAM, or in swap when RAM runs "
                f"out. {ms.fmt_size(total)} in total, {ms.fmt_size(mine)} of it yours. Removing "
                "them frees that memory right away.")
        for mp, n in ms.hidden_usage(items, mounts).items():
            text += (f"\n{ms.fmt_size(n)} of {mp} is not visible as files: running apps keep "
                     "it open (browsers do this) and free it when they quit.")
        self.files_info.set_label(text)
        big = [i for i in items if i.size >= ms.SMALL_ITEM]
        shown = big if self.show_all_files else big[:FILES_SHOWN]
        unused = [i for i in items if i.unused]
        self.remove_all_btn.set_sensitive(bool(unused))
        now = time.time()
        sig = [(i.path, i.size >> 10, i.in_use, i.own, int(now - i.mtime) // 60) for i in shown]
        if sig != self._files_sig:
            self._files_sig = sig
            clear(self.files_box)
            for it in shown:
                self.files_box.append(self._file_row(it, now))
            if not shown:
                self.files_box.append(row("object-select-symbolic", "Nothing to show",
                                          "No files over %s in RAM folders" %
                                          ms.fmt_size(ms.SMALL_ITEM)))
        small = len(items) - len(big)
        rest = len(big) - len(shown)
        notes = []
        if rest:
            notes.append("%s more" % plural(rest, "item"))
        if small:
            notes.append("%s under %s not shown" % (plural(small, "small item"),
                                                     ms.fmt_size(ms.SMALL_ITEM)))
        self.files_note.set_label(", ".join(notes))
        self.show_all_btn.set_visible(len(big) > FILES_SHOWN)
        self.show_all_btn.set_label("Show fewer" if self.show_all_files else
                                    "Show all %d" % len(big))
        return False

    def _file_row(self, it, now):
        parts = [os.path.dirname(it.path)]
        if it.note:
            parts.append(it.note)
        if it.mtime:
            parts.append("changed %s ago" % ms.fmt_age(now - it.mtime))
        if not it.own:
            parts.append("not yours")
        r = row("folder-symbolic" if it.is_dir else "text-x-generic-symbolic", it.name,
                "  ·  ".join(parts))
        if it.in_use:
            r.suffix.append(label("In use", "hb-tm-pill", valign=Gtk.Align.CENTER))
        elif it.sockets:
            r.suffix.append(label("Sockets", "hb-tm-pill", "hb-tm-pill-warn",
                                  valign=Gtk.Align.CENTER))
        r.suffix.append(label(ms.fmt_size(it.size), "hb-tm-sweep-size", xalign=1,
                              width_chars=8))
        b = Gtk.Button(icon_name="user-trash-symbolic", css_classes=["hb-tool"],
                       valign=Gtk.Align.CENTER, tooltip_text="Remove")
        b.connect("clicked", lambda *_: self.confirm_remove(it))
        b.set_sensitive(it.own)
        if not it.own:
            b.set_tooltip_text("It belongs to another user")
        r.suffix.append(b)
        return r

    def confirm_remove(self, it):
        why = ms.removable(it.path)
        if why:
            self.win.notify_toast(why)
            return
        body = f"{it.name} ({ms.fmt_size(it.size)}) at {it.path} will be deleted for good."
        if it.in_use:
            body += "\n\nA running program seems to use it. Removing it may break that program."
        elif it.sockets:
            body += (f"\n\nIt holds {plural(it.sockets, 'socket')}: programs talk through "
                     "these, and may stop working.")
        confirm(self.win, f"Remove {it.name}?", body, "Remove", lambda: self._remove([it]))

    def confirm_remove_unused(self):
        items = self.items
        if items is None:
            self.refresh(files=True)
            return
        unused = [i for i in items if i.unused and not ms.removable(i.path)]
        skipped = len(items) - len(unused)
        if not unused:
            self.win.notify_toast("Nothing to clean up: the files in memory are in use or "
                                  "not yours")
            scroll_to_widget(self.scroller, self.files_anchor)
            return
        mem = sum(i.size for i in unused)
        names = ", ".join(i.name for i in unused[:4])
        if len(unused) > 4:
            names += " and %d more" % (len(unused) - 4)
        body = (f"{plural(len(unused), 'item')} using about {ms.fmt_size(mem)} will be deleted "
                f"for good: {names}.")
        if skipped:
            body += (f"\n\n{plural(skipped, 'item')} in use, holding sockets or of other users "
                     "are left alone.")
        confirm(self.win, "Remove all unused files?", body, "Remove", lambda: self._remove(
            unused, bulk=True))

    def _remove(self, items, bulk=False):
        def work():
            if bulk:
                n, freed, skipped = ms.remove_unused(items)
                msg = f"Removed {plural(n, 'item')}, {ms.fmt_size(freed)} freed"
                if skipped:
                    msg += f"; {len(skipped)} left alone"
            else:
                it = items[0]
                try:
                    left = ms.remove(it.path)
                    msg = f"Removed {it.name}, {ms.fmt_size(it.size)} freed" if not left else \
                        f"Removed most of {it.name}; {left} items could not be deleted"
                except OSError as e:
                    msg = f"Could not remove {it.name}: {e.strerror or e}"
            GLib.idle_add(done, msg)

        def done(msg):
            self.win.notify_toast(msg, timeout=5)
            self._files_sig = None
            self.refresh(files=True)
            return False
        threading.Thread(target=work, daemon=True).start()


# ------------------------------------------------------------------ Settings: memory optimization

class MemoryOptimization:
    """The "Memory optimization" section of the Settings page: zram, swap file,
    zswap, swappiness and cache pressure, applied by the root helper."""

    def __init__(self, page):
        self.page = page
        self.win = page.win
        self.status = None
        self.form = None
        self.pending = []
        self.applying = False
        self.syncing = False
        self.widgets = []
        head = section("Memory optimization")
        self.anchor = head
        self.widgets.append(head)
        intro = Gtk.Box(spacing=6)
        intro.append(label("zram keeps swap in RAM, compressed, so it stays fast. A swap file "
                           "on disk catches what does not fit. zswap compresses pages on their "
                           "way to the swap file; next to zram it adds little.",
                           "hb-tm-sweep-intro", wrap=True, hexpand=True))
        self.widgets.append(intro)
        learn = Gtk.LinkButton(uri=ms.ZRAM_DOCS, label="Learn more about zram",
                               halign=Gtk.Align.START, css_classes=["hb-link"])
        learn.set_margin_bottom(6)
        self.widgets.append(learn)

        # zram-tools
        self.tools_row = row("package-x-generic-symbolic", "zram-tools", "", wrap=True)
        self.tools_switch = switch(False, self._install_toggled)
        self.tools_dd = dropdown(["Keep", "Reinstall", "Remove"], 0, self._tools_picked)
        self.tools_row.suffix.append(self.tools_switch)
        self.tools_row.suffix.append(self.tools_dd)
        # zram
        L = ms.LIMITS
        self.zram_row = row("drive-harddisk-solidstate-symbolic", "zram size", "", wrap=True)
        self.zram_spin = spin(*L["zram"], 0, lambda v: self._changed("zram", v))
        self.zram_row.suffix.append(self.zram_spin)
        self.zram_row.suffix.append(label("% of RAM", "hb-tm-unit"))
        self.algo_row = row("package-x-generic-symbolic", "Compression",
                            "How zram packs pages. zstd fits the most in RAM; lz4 is the "
                            "quickest.", wrap=True)
        self.algo_row.icon.set_from_icon_name("view-paged-symbolic")
        self.algo_dd = dropdown(ms.ALGO_NAMES, 1,
                                lambda i: self._changed("algo", ms.ALGOS[i]))
        self.algo_row.suffix.append(self.algo_dd)
        # swap file
        self.file_row = row("drive-harddisk-symbolic", "Swap file size", "", wrap=True)
        self.file_spin = spin(*L["file"], 4, lambda v: self._changed("file", v))
        self.file_row.suffix.append(self.file_spin)
        self.file_row.suffix.append(label("GB", "hb-tm-unit"))
        # zswap
        self.zswap_row = row("media-flash-symbolic", "zswap", "", wrap=True)
        self.zswap_switch = switch(False, lambda on: self._changed("zswap", on))
        self.zswap_row.suffix.append(self.zswap_switch)
        # vm
        self.swappiness_row = row("power-profile-balanced-symbolic", "Swappiness", "", wrap=True)
        self.swappiness_spin = spin(*L["swappiness"], 60,
                                    lambda v: self._changed("swappiness", v))
        self.swappiness_row.suffix.append(self.swappiness_spin)
        self.swappiness_row.suffix.append(label("", "hb-tm-unit"))
        self.cache_row = row("view-refresh-symbolic", "Cache pressure", "", wrap=True)
        self.cache_spin = spin(*L["cache_pressure"], 100,
                               lambda v: self._changed("cache_pressure", v))
        self.cache_row.suffix.append(self.cache_spin)
        self.cache_row.suffix.append(label("", "hb-tm-unit"))
        self.widgets.append(card_group(self.tools_row, self.zram_row, self.algo_row,
                                       self.file_row, self.zswap_row, self.swappiness_row,
                                       self.cache_row))

        # plan + apply
        plan = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10,
                       css_classes=["hb-tm-sweep-hero"], margin_top=8)
        top = Gtk.Box(spacing=12)
        img = Gtk.Image.new_from_icon_name("security-high-symbolic")
        img.set_pixel_size(18)
        img.set_valign(Gtk.Align.START)
        top.append(img)
        texts = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4, hexpand=True)
        self.plan_title = label("No changes yet", "hb-card-title")
        texts.append(self.plan_title)
        self.plan_label = label("", "hb-tm-plan", wrap=True)
        texts.append(self.plan_label)
        texts.append(label("Changes need your administrator password. They apply at once and "
                           "stay after a restart.", "hb-tm-note", wrap=True))
        top.append(texts)
        buttons = Gtk.Box(spacing=8, valign=Gtk.Align.END)
        buttons.append(button("Reset to defaults", self._reset))
        self.apply_btn = Gtk.Button(css_classes=["hb-accent"], valign=Gtk.Align.CENTER)
        inner = Gtk.Box(spacing=8)
        self.apply_spinner = Adw.Spinner(visible=False)
        inner.append(self.apply_spinner)
        inner.append(Gtk.Label(label="Apply"))
        self.apply_btn.set_child(inner)
        self.apply_btn.connect("clicked", lambda *_: self._apply())
        buttons.append(self.apply_btn)
        top.append(buttons)
        plan.append(top)
        self.widgets.append(plan)

    # ---------------------------------------------------------- state
    def load(self, keep_form=False):
        """Read the system; keep_form keeps what the user typed (timer refresh)."""
        try:
            self.status = ms.swap_status()
        except OSError:
            return
        if self.form is None or not keep_form:
            self.form = ms.swap_form(self.status)
            self._fill_form()
        self._update()

    def _fill_form(self):
        f = self.form
        self.syncing = True
        try:
            self.zram_spin.set_quiet(f["zram"])
            self.algo_dd.set_selected(ms.ALGOS.index(f["algo"]))
            # No swap file yet: 0 keeps "no change" until the value moves.
            self.file_spin.get_adjustment().set_lower(1 if f["file"] else 0)
            self.file_spin.set_quiet(f["file"])
            self.zswap_switch.set_quiet(f["zswap"])
            self.swappiness_spin.set_quiet(f["swappiness"])
            self.cache_spin.set_quiet(f["cache_pressure"])
            self.tools_switch.set_quiet(f["tools"] == "install")
            self.tools_dd.set_selected({"reinstall": 1, "remove": 2}.get(f["tools"], 0))
        finally:
            self.syncing = False

    def _changed(self, key, value):
        if self.syncing or self.form is None:
            return
        self.form[key] = value
        self._update()

    def _install_toggled(self, on):
        if self.syncing or self.form is None:
            return
        self.form["tools"] = "install" if on else None
        self._update()

    def _tools_picked(self, idx):
        if self.syncing or self.form is None:
            return
        self.form["tools"] = (None, "reinstall", "remove")[idx]
        self._update()

    def _reset(self):
        if self.form is None:
            return
        tools = self.form["tools"]
        self.form = dict(ms.DEFAULTS, tools=tools)
        if not self.status["file_size"]:
            self.form["file"] = ms.DEFAULTS["file"]
        self._fill_form()
        self._update()

    def _update(self):
        st, f = self.status, self.form
        if st is None or f is None:
            return
        self.pending = ms.plan_changes(st, f)
        if self.pending:
            self.plan_title.set_label("Waiting to apply")
            self.plan_label.set_label("\n".join("•  " + ms.describe_change(a)
                                                for a in self.pending))
        else:
            self.plan_title.set_label("No changes yet")
            self.plan_label.set_label("Change a setting above, then apply it here.")
        self.apply_btn.set_sensitive(bool(self.pending) and not self.applying)
        installed = st["tools"]
        self.tools_switch.set_visible(not installed)
        self.tools_dd.set_visible(bool(installed))
        if installed:
            self.tools_row.title.set_label("zram-tools")
            set_sub(self.tools_row, "Installed. Sets up zram at every start")
        else:
            self.tools_row.title.set_label("zram-tools is not installed")
            set_sub(self.tools_row, "zram needs it. Turn this on to install it (apt-get) "
                                    "together with the other changes")
        usable = (installed or f["tools"] in ("install", "reinstall")) and f["tools"] != "remove"
        for w in (self.zram_spin, self.algo_dd):
            w.set_sensitive(bool(usable))
        pct = f["zram"]
        now = ""
        if st["zram_on"]:
            now = " Now %s, %s." % (ms.fmt_size(st["zram_size"]), st["algo"])
        set_sub(self.zram_row,
                "Install zram-tools first" if not usable else
                "Off. Swap goes straight to disk." + now if pct == 0 else
                f"Up to {ms.fmt_size(st['ram'] * pct // 100)} of swapped memory kept compressed "
                f"in RAM. 0 turns zram off.{now}")
        if st["file"]:
            where = (f"{st['file']}, {ms.fmt_size(st['file_size'])} now" if st["file_size"]
                     else st["file"])
        else:
            where = "No swap file yet; one is made at /swap.img"
        set_sub(self.file_row, f"{where}  ·  {ms.fmt_size(st['disk_free'])} free on disk")
        self.zswap_switch.switch.set_sensitive(st["zswap"] is not None)
        set_sub(self.zswap_row,
                "This kernel has no zswap" if st["zswap"] is None else
                "Compresses pages before they reach the swap file. With zram on it adds little."
                if pct else "Compresses pages in RAM before they reach the swap file.")
        set_sub(self.swappiness_row, "How early the kernel moves idle pages to swap. 60 is the "
                                     "usual default; 100 to 180 suits zram. Now %d."
                % st["swappiness"])
        set_sub(self.cache_row, "How quickly the kernel drops cached file and folder names. "
                                "100 is the default. Now %d." % st["cache_pressure"])

    def _apply(self):
        args = list(self.pending)
        if not args or self.applying:
            return
        self.applying = True
        self.apply_spinner.set_visible(True)
        self.apply_btn.set_sensitive(False)
        holder = {}

        def done(ok, out, _err):
            p = holder.get("proc")
            code = 0 if ok else (p.get_exit_status() if p is not None else 1)
            self.applying = False
            self.apply_spinner.set_visible(False)
            self.win.notify_toast(ms.swap_result(code, out), timeout=6)
            self.load()
            self.page.app.refresh_now()
        holder["proc"] = run_cmd(ms.swap_command(args), done, mutating=True)

    def on_tick(self):
        if not self.applying:
            self.load(keep_form=True)
