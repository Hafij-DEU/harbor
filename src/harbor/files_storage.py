"""Harbor Files: the "Storage overview" section on This PC.

Every number comes from the real file systems: statvfs() of each mounted
local drive (counted once per device) and dir_size() of the user folders.
"Other" is whatever is used but not inside those folders. The folder scan
runs in a thread and is cached for a minute so going back to This PC does
not rescan.
"""

import math
import os
import threading
import time

import gi

gi.require_version("Gsk", "4.0")
from gi.repository import Gdk, GLib, Graphene, Gsk, Gtk  # noqa: E402

from . import files_core as fc  # noqa: E402
from . import files_tint  # noqa: E402
from .common import human_size  # noqa: E402

CATEGORY_COLORS = ("#3b82f6", "#a855f7", "#ec4899", "#f59e0b", "#22c55e", "#14b8a6")
OTHER_COLOR = "#9ca3af"
CACHE_SECONDS = 60
_cache = {"at": 0.0, "key": None, "sizes": None}


def _rgba(spec, alpha=1.0):
    c = Gdk.RGBA()
    c.parse(spec)
    c.alpha = alpha
    return c


def volume_totals(paths):
    """(total, used, free) bytes over the given mount points, counting each
    device once. Unreadable paths are skipped."""
    seen = set()
    total = used = free = 0
    for p in paths:
        try:
            dev = os.stat(p).st_dev
            if dev in seen:
                continue
            st = os.statvfs(p)
        except OSError:
            continue
        seen.add(dev)
        size = st.f_blocks * st.f_frsize
        if not size:
            continue
        avail = st.f_bavail * st.f_frsize
        total += size
        free += avail
        used += size - st.f_bfree * st.f_frsize
    return total, used, free


def category_sizes(folders, cancelled=lambda: False, budget=6.0):
    """[(label, bytes, complete)] for [(label, path)]; missing folders skipped."""
    out = []
    for label, path in folders:
        if cancelled():
            return []
        if not os.path.isdir(path):
            continue
        size, complete = fc.dir_size(path, cancelled, budget=budget)
        if size is None:
            continue
        out.append((label, size, complete))
    return out


def with_other(cats, used):
    """Add an "Other" row so the parts add up to used space."""
    rest = max(0, used - sum(c[1] for c in cats))
    return list(cats) + [("Other", rest, True)]


class Donut(Gtk.Widget):
    """A ring: grey track and an accent arc for the used fraction."""

    def __init__(self, size=112, width=12):
        super().__init__()
        self.fraction = 0.0
        self.ring = width
        self.set_size_request(size, size)

    def set_fraction(self, f):
        self.fraction = max(0.0, min(1.0, f))
        self.queue_draw()

    def do_snapshot(self, snap):
        w, h = self.get_width(), self.get_height()
        cx, cy = w / 2, h / 2
        r = min(w, h) / 2 - self.ring / 2 - 1
        stroke = Gsk.Stroke.new(self.ring)
        fg = self.get_color()
        track = Gdk.RGBA()
        track.red, track.green, track.blue, track.alpha = fg.red, fg.green, fg.blue, 0.12
        circle = "M %f %f A %f %f 0 1 1 %f %f A %f %f 0 1 1 %f %f" % (
            cx, cy - r, r, r, cx, cy + r, r, r, cx, cy - r)
        snap.append_stroke(Gsk.Path.parse(circle), stroke, track)
        f = self.fraction
        if f <= 0.001:
            return
        accent = _rgba(files_tint.accent_hex() or "#3584e4")
        if f >= 0.999:
            path = circle
        else:
            a = 2 * math.pi * f
            path = "M %f %f A %f %f 0 %d 1 %f %f" % (
                cx, cy - r, r, r, 1 if a > math.pi else 0,
                cx + r * math.sin(a), cy - r * math.cos(a))
        stroke.set_line_cap(Gsk.LineCap.ROUND)
        snap.append_stroke(Gsk.Path.parse(path), stroke, accent)


class StackBar(Gtk.Widget):
    """One horizontal bar split into coloured parts."""

    def __init__(self, height=10):
        super().__init__(hexpand=True, valign=Gtk.Align.CENTER)
        self.parts = []
        self.set_size_request(160, height)

    def set_parts(self, parts):
        self.parts = [(v, c) for v, c in parts if v > 0]
        self.queue_draw()

    def do_snapshot(self, snap):
        w, h = self.get_width(), self.get_height()
        total = sum(v for v, _c in self.parts)
        if w <= h:
            return
        rect = Graphene.Rect().init(0, 0, w, h)
        r = h / 2
        # Gsk.RoundedRect is unusable from Python; clip with a path instead.
        shape = Gsk.Path.parse("M %f 0 L %f 0 A %f %f 0 0 1 %f %f L %f %f A %f %f 0 0 1 %f 0 Z" % (
            r, w - r, r, r, w - r, h, r, h, r, r, r))
        snap.push_fill(shape, Gsk.FillRule.WINDING)
        fg = self.get_color()
        bg = Gdk.RGBA()
        bg.red, bg.green, bg.blue, bg.alpha = fg.red, fg.green, fg.blue, 0.10
        snap.append_color(bg, rect)
        x = 0.0
        for v, color in self.parts:
            pw = w * v / total if total else 0
            snap.append_color(_rgba(color), Graphene.Rect().init(x, 0, pw + 0.5, h))
            x += pw
        snap.pop()


def _dot(color):
    d = Gtk.Box(css_classes=["hbf-legend-dot"], valign=Gtk.Align.CENTER)
    prov = Gtk.CssProvider()
    prov.load_from_string(".hbf-legend-dot { background: %s; }" % color)
    d.get_style_context().add_provider(prov, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
    return d


def default_folders():
    """[(kind, path)] of the user folders shown in the breakdown."""
    return [(kind, p) for kind, p, _i in fc.user_folders()
            if kind != "Desktop" and os.path.isdir(p)]


def storage_button(get_mounts):
    """Small status-bar button; its popover holds the storage overview and
    refreshes each time it opens. get_mounts() -> [local mount paths]."""
    pop = Gtk.Popover(css_classes=["hbf-storage-pop"])
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, margin_start=14,
                  margin_end=14, margin_top=10, margin_bottom=12)
    box.append(_lab("Storage overview", "heading"))
    ov = StorageOverview()
    ov.set_size_request(640, -1)
    box.append(ov)
    pop.set_child(box)
    pop.connect("show", lambda *_: ov.refresh(get_mounts(), default_folders()))
    btn = Gtk.MenuButton(icon_name="drive-harddisk-symbolic", popover=pop,
                         css_classes=["flat", "hbf-storage-btn"], valign=Gtk.Align.CENTER,
                         tooltip_text="Storage overview")
    btn.overview = ov
    return btn


def _lab(text, *cls, xalign=0):
    return Gtk.Label(label=text, xalign=xalign, css_classes=list(cls))


class StorageOverview(Gtk.Box):
    def __init__(self):
        super().__init__(spacing=36, css_classes=["hbf-storage"], margin_top=4)
        self.gen = 0
        ov = Gtk.Overlay()
        self.donut = Donut()
        ov.set_child(self.donut)
        mid = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, valign=Gtk.Align.CENTER,
                      halign=Gtk.Align.CENTER)
        self.pct = _lab("–", "hbf-donut-pct", xalign=0.5)
        mid.append(self.pct)
        mid.append(_lab("Used", "hb-dim", "hb-small", xalign=0.5))
        ov.add_overlay(mid)
        self.append(ov)

        nums = Gtk.Grid(column_spacing=14, row_spacing=8, valign=Gtk.Align.CENTER)
        self.num = {}
        for r, (key, title, color) in enumerate((("used", "Used space", "@accent"),
                                                ("free", "Free space", None),
                                                ("total", "Total space", None))):
            if color == "@accent":
                dot = Gtk.Box(css_classes=["hbf-legend-dot", "hbf-dot-accent"],
                              valign=Gtk.Align.CENTER)
            else:
                dot = Gtk.Box(css_classes=["hbf-legend-dot", "hbf-dot-" + key],
                              valign=Gtk.Align.CENTER)
            nums.attach(dot, 0, r, 1, 1)
            nums.attach(_lab(title + ":"), 1, r, 1, 1)
            self.num[key] = _lab("–", "numeric")
            nums.attach(self.num[key], 2, r, 1, 1)
        self.append(nums)
        self.append(Gtk.Separator(orientation=Gtk.Orientation.VERTICAL))

        right = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10, hexpand=True,
                        valign=Gtk.Align.CENTER)
        self.bar = StackBar()
        right.append(self.bar)
        self.legend = Gtk.Grid(column_spacing=14, row_spacing=4)
        right.append(self.legend)
        self.append(right)

    def refresh(self, mount_paths, folders):
        """mount_paths: local mount points. folders: [(label, path)]."""
        self.gen += 1
        gen = self.gen
        total, used, free = volume_totals(mount_paths)
        self.num["used"].set_label(human_size(used) if total else "–")
        self.num["free"].set_label(human_size(free) if total else "–")
        self.num["total"].set_label(human_size(total) if total else "–")
        frac = used / total if total else 0.0
        self.donut.set_fraction(frac)
        self.pct.set_label("%d%%" % round(frac * 100) if total else "–")
        key = tuple(p for _l, p in folders)
        if _cache["key"] == key and time.monotonic() - _cache["at"] < CACHE_SECONDS:
            self._fill(gen, _cache["sizes"], used)
            return
        self._fill(gen, None, used, labels=[lab for lab, _p in folders])

        def work():
            sizes = category_sizes(folders, lambda: gen != self.gen)
            if gen == self.gen:
                _cache.update(at=time.monotonic(), key=key, sizes=sizes)
                GLib.idle_add(self._fill, gen, sizes, used)
        threading.Thread(target=work, daemon=True, name="hb-storage").start()

    def _fill(self, gen, sizes, used, labels=()):
        if gen != self.gen:
            return False
        child = self.legend.get_first_child()
        while child is not None:
            nxt = child.get_next_sibling()
            self.legend.remove(child)
            child = nxt
        if sizes is None:
            rows = [(lab, None, True) for lab in labels]
            self.bar.set_parts([])
        else:
            rows = with_other(sizes, used)
        parts = []
        half = (len(rows) + 1) // 2
        for i, (lab, size, complete) in enumerate(rows):
            color = OTHER_COLOR if lab == "Other" else CATEGORY_COLORS[i % len(CATEGORY_COLORS)]
            parts.append((size or 0, color))
            col, r = (0, i) if i < half else (3, i - half)
            self.legend.attach(_dot(color), col, r, 1, 1)
            self.legend.attach(_lab(lab), col + 1, r, 1, 1)
            text = "…" if size is None else human_size(size) + ("" if complete else "+")
            self.legend.attach(_lab(text, "numeric", "hb-dim", xalign=1), col + 2, r, 1, 1)
        if sizes is not None:
            self.bar.set_parts(parts)
        return False
