"""Performance, App history, Startup apps, Services and Settings pages."""

import math
import os
import re
import time

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gio, GLib, Gtk, Pango  # noqa: E402

from . import taskmgr_proc as tp  # noqa: E402
from . import taskmgr_store as ts  # noqa: E402
from .common import (Card, about_dialog, card_group, dropdown, have, human_size,  # noqa: E402
                     label, page_box, run_cmd, section, spawn, switch)
from .taskmgr_graph import CompositionBar, Graph  # noqa: E402
from .taskmgr_model import Item, sync_store  # noqa: E402
from .taskmgr_ui import (Col, Page, Table, confirm, copy_text, info_dialog,  # noqa: E402
                         key_sorter, scroll_to_widget, spin, tool_button)

GB = 1024.0 ** 3


def gb(n, d=1):
    return ("%." + str(d) + "f GB") % (n / GB)


def ghz(khz):
    return "%.2f GHz" % (khz / 1e6) if khz else "—"


def rate_bits(bps_bytes):
    """Network rate like Windows: Kbps / Mbps / Gbps."""
    v = bps_bytes * 8
    if v < 1e6:
        return "%.0f Kbps" % (v / 1e3)
    if v < 1e9:
        return "%.1f Mbps" % (v / 1e6)
    return "%.2f Gbps" % (v / 1e9)


def rate_bytes(r):
    if r < 1024 * 1024:
        return "%.0f KB/s" % (r / 1024)
    return "%.1f MB/s" % (r / 1024 / 1024)


# ------------------------------------------------------------------ performance

class StatBlock:
    """Windows-style stats: big values (label over value) on the left,
    key: value pairs on the right."""

    def __init__(self):
        self.box = Gtk.Box(spacing=40, margin_top=14)
        self.big = Gtk.Grid(column_spacing=34, row_spacing=12, valign=Gtk.Align.START)
        self.kv = Gtk.Grid(column_spacing=24, row_spacing=4, valign=Gtk.Align.START)
        self.box.append(self.big)
        self.box.append(self.kv)
        self.values = {}
        self.order = []
        self._bi = 0
        self._ki = 0

    def add_big(self, key, title, col=None, row=None):
        cell = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=1)
        cell.append(label(title, "hb-tm-stat-label"))
        val = label("—", "hb-tm-stat-value")
        cell.append(val)
        if col is None:
            col, row = self._bi % 3, self._bi // 3
            self._bi += 1
        self.big.attach(cell, col, row, 1, 1)
        self.values[key] = (title, val)
        self.order.append(key)

    def add_kv(self, key, title):
        self.kv.attach(label(title + ":", "hb-tm-kv-label"), 0, self._ki, 1, 1)
        val = label("—", "hb-tm-kv-value", selectable=False,
                    ellipsize=Pango.EllipsizeMode.END)
        val.set_max_width_chars(34)
        self.kv.attach(val, 1, self._ki, 1, 1)
        self._ki += 1
        self.values[key] = (title, val)
        self.order.append(key)

    def set(self, key, text):
        v = self.values.get(key)
        if v and v[1].get_label() != text:
            v[1].set_label(text)

    def text(self):
        return "\n".join("%s: %s" % (self.values[k][0], self.values[k][1].get_label())
                         for k in self.order)


class Device:
    """One entry of the Performance list and its detail view."""

    kind = ""
    graph_caption = "% Utilization"
    icon = "computer-symbolic"

    def __init__(self, page, key, name):
        self.page = page
        self.app = page.app
        self.sys = page.app.sys
        self.key = key
        self.name = name
        # sidebar row
        self.spark = Graph(grid=False, width=72, height=50)
        self.row_name = label(name, "hb-tm-devname", ellipsize=Pango.EllipsizeMode.END)
        self.row_val = label("", "hb-tm-devval", ellipsize=Pango.EllipsizeMode.END)
        self.row_val.set_lines(2)
        texts = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, valign=Gtk.Align.CENTER,
                        hexpand=True)
        texts.append(self.row_name)
        texts.append(self.row_val)
        rb = Gtk.Box(spacing=12)
        rb.append(self.spark)
        rb.append(texts)
        self.row = Gtk.ListBoxRow(child=rb)
        self.row.key = key
        self.view = None

    # --- detail view scaffolding
    def build(self):
        v = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, margin_start=22, margin_end=22,
                    margin_top=6, margin_bottom=16)
        head = Gtk.Box(spacing=12)
        self.title = label(self.name, "hb-tm-perf-title", ellipsize=Pango.EllipsizeMode.END)
        head.append(self.title)
        head.append(Gtk.Box(hexpand=True))
        self.model = label("", "hb-tm-perf-model", xalign=1, ellipsize=Pango.EllipsizeMode.END,
                           valign=Gtk.Align.CENTER)
        head.append(self.model)
        self.extend_head(head)
        menu = Gio.Menu()
        menu.append("Copy", "perf.copy")
        self.extend_menu(menu)
        mb = Gtk.MenuButton(icon_name="view-more-horizontal-symbolic", menu_model=menu,
                            css_classes=["hb-tool"], valign=Gtk.Align.CENTER,
                            tooltip_text="More options")
        mb.set_has_frame(False)
        head.append(mb)
        v.append(head)
        self.stats = StatBlock()
        self.build_body(v)
        v.append(self.stats.box)
        self.build_stats(self.stats)
        self.build_after(v)
        sc = Gtk.ScrolledWindow(child=v, vexpand=True, hexpand=True)
        sc.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.view = sc
        g = Gtk.GestureClick(button=3)
        g.connect("pressed", lambda _g, _n, x, y: self.page.popup_menu(menu, v, x, y))
        v.add_controller(g)
        return sc

    def axis(self, left, right):
        b = Gtk.Box(margin_top=4, margin_bottom=2)
        ll = label(left, "hb-tm-axis", hexpand=True)
        rl = label(right, "hb-tm-axis", xalign=1)
        b.append(ll)
        b.append(rl)
        return b, ll, rl

    def graph_block(self, parent, caption, graph, top_right="100%", bottom_left=None):
        top, self._cap_l, self._cap_r = self.axis(caption, top_right)
        parent.append(top)
        parent.append(graph)
        bottom, self._bl, self._br = self.axis(bottom_left or self.page.seconds_label(), "0")
        parent.append(bottom)
        return self._cap_r, self._bl

    def extend_menu(self, menu):
        pass

    def extend_head(self, head):
        pass

    def build_after(self, v):
        pass

    def build_body(self, v):
        self.graph = Graph(height=260)
        self.graph.set_vexpand(True)
        self.top_r, self.bottom_l = self.graph_block(v, self.graph_caption, self.graph)

    def build_stats(self, s):
        pass

    def update_row(self):
        pass

    def update_view(self):
        pass

    def copy_text(self):
        return "%s\n%s\n%s" % (self.name, self.model.get_label() if self.view else "",
                               self.stats.text() if self.view else "")


class CPUDevice(Device):
    kind = "cpu"

    def __init__(self, page):
        super().__init__(page, "cpu", "CPU")

    def extend_menu(self, menu):
        sub = Gio.Menu()
        sub.append("Overall utilization", "perf.cpu-graph::overall")
        sub.append("Logical processors", "perf.cpu-graph::cores")
        menu.prepend_submenu("Change graph to", sub)

    def build_body(self, v):
        self.graph = Graph(height=260)
        self.graph.set_vexpand(True)
        self.graph_stack = Gtk.Stack(vexpand=True)
        self.graph_stack.add_named(self.graph, "overall")
        n = len(self.sys.cores) or os.cpu_count() or 1
        base = max(1, math.ceil(math.sqrt(n)))
        cols = base
        while n % cols and cols < base * 2:
            cols += 1
        if n % cols:
            cols = base
        grid = Gtk.Grid(column_spacing=4, row_spacing=4, column_homogeneous=True,
                        row_homogeneous=True)
        self.core_graphs = []
        for i in range(n):
            g = Graph(grid=True, height=40)
            g.set_hexpand(True)
            g.set_vexpand(True)
            g.set_tooltip_text("CPU %d" % i)
            grid.attach(g, i % cols, i // cols, 1, 1)
            self.core_graphs.append(g)
        self.graph_stack.add_named(grid, "cores")
        top, _l, self.top_r = self.axis(self.graph_caption, "100%")
        v.append(top)
        v.append(self.graph_stack)
        bottom, self.bottom_l, _r = self.axis(self.page.seconds_label(), "0")
        v.append(bottom)
        self.set_mode(self.app.settings["cpu_graph"])

    def set_mode(self, mode):
        mode = mode if mode in ("overall", "cores") else "overall"
        self.graph_stack.set_visible_child_name(mode)
        self.top_r.set_label("100%")

    def build_stats(self, s):
        info = self.sys.cpu_info
        self.model.set_label(info["model"])
        s.add_big("util", "Utilization", 0, 0)
        s.add_big("speed", "Speed", 1, 0)
        s.add_big("procs", "Processes", 0, 1)
        s.add_big("threads", "Threads", 1, 1)
        s.add_big("handles", "File handles", 2, 1)
        s.add_big("uptime", "Up time", 0, 2)
        s.add_kv("base", "Base speed")
        s.add_kv("sockets", "Sockets")
        s.add_kv("cores", "Cores")
        s.add_kv("logical", "Logical processors")
        s.add_kv("virt", "Virtualization")
        for lvl in ("L1", "L2", "L3"):
            s.add_kv(lvl, "%s cache" % lvl)
        s.set("base", ghz(self.sys.base_speed()))
        s.set("sockets", str(info["sockets"]))
        s.set("cores", str(info["cores"]))
        s.set("logical", str(info["logical"]))
        s.set("virt", ("Supported (%s)" % info["virtualization"]) if info["virtualization"]
              else "Not supported")
        caches = self.sys.caches
        for lvl in ("L1", "L2", "L3"):
            if lvl in caches:
                s.set(lvl, human_size(caches[lvl]).replace(".0 ", " "))

    def update_row(self):
        self.spark.set_series(self.sys.cpu_hist)
        self.row_val.set_label("%d%%  %s" % (round(self.sys.cpu), ghz(self._speed())))

    def _speed(self):
        now = time.monotonic()
        if now - getattr(self, "_speed_t", 0) > 2:
            self._speed_t = now
            self._speed_v = self.sys.cpu_speed()
        return self._speed_v

    def update_view(self):
        s = self.stats
        procs = self.app.proc_sampler.procs
        s.set("util", "%d%%" % round(self.sys.cpu))
        s.set("speed", ghz(self._speed()))
        user = [p for p in procs.values() if not p.kernel]
        s.set("procs", str(len(user)))
        s.set("threads", "{:,}".format(sum(p.threads for p in procs.values())))
        fnr = tp.read_text("/proc/sys/fs/file-nr").split()
        s.set("handles", "{:,}".format(int(fnr[0])) if fnr else "—")
        s.set("uptime", tp.format_uptime(self.sys.uptime()))
        if self.graph_stack.get_visible_child_name() == "cores":
            for g, h in zip(self.core_graphs, self.sys.core_hist):
                g.set_series(h)
        else:
            self.graph.set_series(self.sys.cpu_hist)


class MemoryDevice(Device):
    kind = "memory"
    graph_caption = "Memory usage"

    # (key, title, hint) of the four big numbers under the composition bar
    MAIN = (("used", "In use", "Used by apps and the system"),
            ("avail", "Available", "Free to use right now"),
            ("cached", "Cached", "Recent files kept for speed; given back when needed"),
            ("swap", "Swap used", "Moved out of RAM to make room"))
    # More details: two label/value columns
    MORE_L = (("total", "Total"), ("committed", "Promised to apps"),
              ("kernel", "Kernel memory"), ("buffers", "Buffers"), ("shared", "Shared"))
    MORE_R = (("swaptotal", "Swap size"), ("zcompr", "Compressed size"),
              ("zused", "zram RAM used"), ("zdisk", "zram disk size"),
              ("zshare", "zram share of RAM"))
    ZRAM_KEYS = ("zcompr", "zused", "zdisk", "zshare")

    def __init__(self, page):
        super().__init__(page, "memory", "Memory")

    def build_body(self, v):
        self.graph = Graph(height=220)
        self.graph.set_vexpand(True)
        self.top_r, self.bottom_l = self.graph_block(v, self.graph_caption, self.graph,
                                                     top_right="")
        cap = label("Memory composition", "hb-tm-axis", margin_top=10, margin_bottom=2)
        v.append(cap)
        self.bar = CompositionBar(height=44)
        v.append(self.bar)
        legend = Gtk.Box(spacing=22, margin_top=7)
        self.legend = {}
        for key, cls, text in (("used", "hb-tm-legend-solid", "In use"),
                               ("cached", "hb-tm-legend-cached", "Cached"),
                               ("free", "hb-tm-legend-free", "Free")):
            b = Gtk.Box(spacing=6)
            b.append(Gtk.Box(css_classes=["hb-tm-legend", cls], valign=Gtk.Align.CENTER))
            b.append(label(text, "hb-tm-legend-name"))
            val = label("—", "hb-tm-legend-value")
            b.append(val)
            legend.append(b)
            self.legend[key] = (text, val)
        v.append(legend)

    def build_stats(self, s):
        # The generic StatBlock is not used here; build_after lays out the
        # simpler memory view.
        s.box.set_visible(False)

    def extend_head(self, head):
        b = Gtk.Button(label="Free up memory", css_classes=["hb-plain"],
                       valign=Gtk.Align.CENTER, margin_start=6,
                       tooltip_text="Open Memory Sweep")
        b.connect("clicked", lambda *_: self.page.win.show_page("sweep"))
        head.append(b)

    def _big(self, key, title, hint):
        cell = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=1, hexpand=True)
        cell.append(label(title, "hb-tm-stat-label"))
        row = Gtk.Box(spacing=5)
        val = label("—", "hb-tm-stat-value", valign=Gtk.Align.BASELINE_FILL)
        of = label("", "hb-tm-stat-of", valign=Gtk.Align.BASELINE_FILL)
        of.set_visible(False)
        row.append(val)
        row.append(of)
        cell.append(row)
        h = label(hint, "hb-tm-stat-hint", wrap=True, margin_top=2)
        h.set_wrap_mode(Pango.WrapMode.WORD_CHAR)
        h.set_max_width_chars(28)
        cell.append(h)
        self.main[key] = (title, val, of)
        return cell

    def build_after(self, v):
        self.main = {}
        # four big numbers: two pairs in a flow box, so they sit in one row
        # when there is room and fold to 2x2 when the window is narrow
        flow = Gtk.FlowBox(selection_mode=Gtk.SelectionMode.NONE, homogeneous=True,
                           min_children_per_line=1, max_children_per_line=2,
                           column_spacing=28, row_spacing=16, margin_top=18,
                           css_classes=["hb-tm-flat"])
        for pair in (self.MAIN[:2], self.MAIN[2:]):
            pb = Gtk.Box(spacing=28, homogeneous=True)
            for key, title, hint in pair:
                pb.append(self._big(key, title, hint))
            flow.append(pb)
        for child in (flow.get_child_at_index(0), flow.get_child_at_index(1)):
            child.set_focusable(False)
        v.append(flow)

        # zram: one short plain-language card
        self.zram_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=3,
                                margin_top=20, css_classes=["hb-tm-mem-card"])
        self.zram_box.append(label("Compressed memory", "hb-tm-mem-card-title"))
        self.zram_line = label("", "hb-tm-mem-card-text", wrap=True, margin_top=2)
        self.zram_line.set_wrap_mode(Pango.WrapMode.WORD_CHAR)
        self.zram_box.append(self.zram_line)
        self.zram_meta = label("", "hb-tm-stat-hint")
        self.zram_box.append(self.zram_meta)
        self.zram_box.set_visible(False)
        v.append(self.zram_box)

        # everything else, collapsed
        self.more_vals = {}
        cols = Gtk.Box(spacing=40, margin_top=10, margin_start=16, homogeneous=True)
        self.more_rows = {}
        for spec in (self.MORE_L, self.MORE_R):
            g = Gtk.Grid(column_spacing=20, row_spacing=5, valign=Gtk.Align.START)
            for i, (key, title) in enumerate(spec):
                k = label(title, "hb-tm-kv-label")
                val = label("—", "hb-tm-kv-value", xalign=0, hexpand=True,
                            ellipsize=Pango.EllipsizeMode.END)
                g.attach(k, 0, i, 1, 1)
                g.attach(val, 1, i, 1, 1)
                self.more_vals[key] = (title, val)
                self.more_rows[key] = (k, val)
            cols.append(g)
        self.more = Gtk.Expander(child=cols, css_classes=["hb-tm-more"])
        self.more.set_label_widget(label("More details", "hb-tm-mem-card-title"))
        card = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, margin_top=14,
                       css_classes=["hb-tm-mem-card", "hb-tm-more-card"])
        card.append(self.more)
        v.append(card)

    def _set(self, lbl, text):
        if lbl.get_label() != text:
            lbl.set_label(text)

    def _set_main(self, key, value, of=""):
        _t, val, ofl = self.main[key]
        self._set(val, value)
        self._set(ofl, of)
        ofl.set_visible(bool(of))

    def _zram(self):
        """(original, compressed, memory used) bytes over all zram devices."""
        zs = self.sys.zram
        return (sum(d["orig"] for d in zs), sum(d["compr"] for d in zs),
                sum(d["mem_used"] for d in zs))

    def _update_zram(self, tot):
        zs = self.sys.zram
        self.zram_box.set_visible(bool(zs))
        for key in self.ZRAM_KEYS:
            for w in self.more_rows[key]:
                w.set_visible(bool(zs))
        if not zs:
            return
        orig, comp, used = self._zram()
        if orig and used:
            text = ("%s of swapped data is packed into %s of RAM, %.1f× smaller."
                    % (human_size(orig), human_size(used), orig / used))
        else:
            text = "Ready to use. Nothing is packed yet."
        self._set(self.zram_line, text)
        algos = sorted({d["algo"] for d in zs if d["algo"]})
        self._set(self.zram_meta, " · ".join(algos + [d["name"] for d in zs]))
        mv = self.more_vals
        self._set(mv["zcompr"][1], human_size(comp))
        self._set(mv["zused"][1], human_size(used))
        self._set(mv["zdisk"][1], gb(sum(d["disksize"] for d in zs)))
        self._set(mv["zshare"][1], "%.1f%%" % (100 * used / tot))

    def copy_text(self):
        if self.view is None:
            return "%s\n" % self.name
        lines = [self.name, self.model.get_label()]
        lines += ["%s: %s" % (t, v.get_label()) for t, v in self.legend.values()]
        for key, _title, _hint in self.MAIN:
            t, val, of = self.main[key]
            lines.append("%s: %s" % (t, " ".join(x for x in (val.get_label(),
                                                             of.get_label()) if x)))
        zram = bool(self.sys.zram)
        if zram:
            lines += ["Compressed memory: " + self.zram_line.get_label(),
                      "Compressed memory: " + self.zram_meta.get_label()]
        for key, (t, v) in self.more_vals.items():
            if zram or key not in self.ZRAM_KEYS:
                lines.append("%s: %s" % (t, v.get_label()))
        return "\n".join(lines)

    def update_row(self):
        m = self.sys.mem_summary()
        self.spark.set_series(self.sys.mem_hist)
        tot = m["total"] or 1
        self.row_val.set_label("%.1f/%s (%d%%)" % (m["used"] / GB, gb(tot),
                                                    round(100 * m["used"] / tot)))

    def update_view(self):
        m = self.sys.mem_summary()
        tot = m["total"] or 1
        self.model.set_label(gb(tot))
        self.top_r.set_label(gb(tot))
        self.graph.set_series(self.sys.mem_hist)
        self.bar.set_parts(m["used"] / tot, m["cached"] / tot)
        # legend matches the bar: used, cached, and what is left
        used = min(m["used"], tot)
        cached = min(m["cached"], tot - used)
        self._set(self.legend["used"][1], gb(used))
        self._set(self.legend["cached"][1], gb(cached))
        self._set(self.legend["free"][1], gb(max(0, tot - used - cached)))
        self._set_main("used", gb(m["used"]), "of " + gb(tot))
        self._set_main("avail", gb(m["available"]))
        self._set_main("cached", gb(m["cached"]))
        self._set_main("swap", gb(m["swap_used"]),
                       "of " + gb(m["swap_total"]) if m["swap_total"] else "")
        mv = self.more_vals
        self._set(mv["total"][1], gb(tot))
        self._set(mv["committed"][1], "%.1f / %.1f GB" % (m["committed"] / GB,
                                                          m["commit_limit"] / GB))
        self._set(mv["kernel"][1], human_size(m["kernel"]))
        self._set(mv["buffers"][1], human_size(m["buffers"]))
        self._set(mv["shared"][1], human_size(m["shmem"]))
        self._set(mv["swaptotal"][1], gb(m["swap_total"]))
        self._update_zram(tot)


class SwapDevice(Device):
    kind = "swap"
    graph_caption = "Swap usage"
    icon = "drive-harddisk-symbolic"

    def __init__(self, page):
        super().__init__(page, "swap", "Swap")

    def build_body(self, v):
        self.graph = Graph(height=180)
        self.graph.set_vexpand(True)
        self.top_r, self.bottom_l = self.graph_block(v, self.graph_caption, self.graph,
                                                     top_right="")
        self.io = Graph(height=90, auto_floor=1024 * 1024)
        top, _l, self.io_max = self.axis("Swap activity (in solid, out dashed)", "")
        top.set_margin_top(10)
        v.append(top)
        v.append(self.io)
        bottom, self.bottom_l2, _r = self.axis(self.page.seconds_label(), "0")
        v.append(bottom)

    def build_stats(self, s):
        s.add_big("used", "In use", 0, 0)
        s.add_big("free", "Available", 1, 0)
        s.add_big("in", "Swap in", 0, 1)
        s.add_big("out", "Swap out", 1, 1)
        s.add_kv("total", "Total")
        s.add_kv("devices", "Devices")
        s.add_kv("swappiness", "Swappiness")

    def build_after(self, v):
        v.append(label("Swap devices", "hb-tm-perf-sub", margin_top=18))
        self.grid = Gtk.Grid(column_spacing=28, row_spacing=4, margin_top=6)
        v.append(self.grid)
        self._grid_sig = None

    def _totals(self):
        sw = self.sys.swaps
        return sum(d["size"] for d in sw), sum(d["used"] for d in sw)

    def update_row(self):
        self.spark.set_series(self.sys.swap_hist)
        tot, used = self._totals()
        if not tot:
            self.row_val.set_label("Off")
            return
        self.row_val.set_label("%.1f/%s (%d%%)" % (used / GB, gb(tot), round(100 * used / tot)))

    def _fill_grid(self):
        rows = [(d["name"], tp.swap_kind_name(d["kind"]), d["size"], d["used"], d["priority"])
                for d in self.sys.swaps]
        sig = [(n, k, sz, u >> 20, pr) for n, k, sz, u, pr in rows]
        if sig == self._grid_sig:
            return
        self._grid_sig = sig
        while (c := self.grid.get_first_child()) is not None:
            self.grid.remove(c)
        for col, (text, right) in enumerate((("Name", False), ("Type", False), ("Size", True),
                                             ("Used", True), ("Priority", True))):
            self.grid.attach(label(text, "hb-tm-th", xalign=1 if right else 0), col, 0, 1, 1)
        if not rows:
            self.grid.attach(label("No swap is active", "hb-tm-td"), 0, 1, 5, 1)
        for r, (name, kind, size, used, prio) in enumerate(rows, start=1):
            cells = (name, kind, gb(size, 2), "%s (%d%%)" % (
                human_size(used), round(100 * used / size) if size else 0), str(prio))
            for col, text in enumerate(cells):
                lab = label(text, "hb-tm-td", xalign=0 if col < 2 else 1,
                            selectable=col == 0, ellipsize=Pango.EllipsizeMode.MIDDLE)
                if col == 0:
                    lab.set_hexpand(False)
                    lab.set_max_width_chars(28)
                self.grid.attach(lab, col, r, 1, 1)

    def update_view(self):
        tot, used = self._totals()
        s = self.stats
        sin, sout = self.sys.swap_now
        self.model.set_label(gb(tot) if tot else "Off")
        self.top_r.set_label(gb(tot) if tot else "")
        self.graph.set_series(self.sys.swap_hist)
        i, o = self.sys.swap_io_hist
        self.io.set_series(i, o)
        self.io_max.set_label(rate_bytes(self.io.current_max()))
        s.set("used", "%s (%d%%)" % (gb(used), round(100 * used / tot)) if tot else "—")
        s.set("free", gb(tot - used) if tot else "—")
        s.set("in", rate_bytes(sin))
        s.set("out", rate_bytes(sout))
        s.set("total", gb(tot))
        s.set("devices", str(len(self.sys.swaps)))
        s.set("swappiness", tp.read_text("/proc/sys/vm/swappiness").strip() or "—")
        self._fill_grid()

    def copy_text(self):
        text = super().copy_text()
        for d in self.sys.swaps:
            text += "\n%s: %s, %s, %s used, priority %s" % (
                d["name"], tp.swap_kind_name(d["kind"]), gb(d["size"], 2),
                human_size(d["used"]), d["priority"])
        return text


def _mounts():
    out = []
    for line in tp.read_text("/proc/mounts").splitlines():
        p = line.split()
        if len(p) >= 3 and p[0].startswith("/dev/"):
            out.append((p[0], p[1].replace("\\040", " "), p[2]))
    return out


def _disk_of(dev):
    """Whole-disk name of a /dev partition path (nvme0n1p2 -> nvme0n1)."""
    name = os.path.basename(os.path.realpath(dev))
    path = "/sys/class/block/%s" % name
    if os.path.exists(path + "/partition"):
        return os.path.basename(os.path.dirname(os.path.realpath(path)))
    slaves = path + "/slaves"
    if os.path.isdir(slaves):
        for s in os.listdir(slaves):
            return _disk_of("/dev/" + s)
    return name


class DiskDevice(Device):
    kind = "disk"
    graph_caption = "Active time"

    def __init__(self, page, index, info):
        self.info = info
        super().__init__(page, "disk%d" % index, "Disk %d (%s)" % (index, info["name"]))
        self.dev = info["name"]
        self._fmt_t = 0

    def build_body(self, v):
        self.graph = Graph(height=180)
        self.graph.set_vexpand(True)
        self.top_r, self.bottom_l = self.graph_block(v, self.graph_caption, self.graph)
        self.rw = Graph(height=90, auto_floor=1024 * 1024)
        top, _l, self.rw_max = self.axis("Disk transfer rate (read solid, write dashed)", "")
        top.set_margin_top(10)
        v.append(top)
        v.append(self.rw)
        bottom, self.bottom_l2, _r = self.axis(self.page.seconds_label(), "0")
        v.append(bottom)

    def build_stats(self, s):
        self.model.set_label(self.info["model"])
        s.add_big("active", "Active time", 0, 0)
        s.add_big("resp", "Average response time", 1, 0)
        s.add_big("read", "Read speed", 0, 1)
        s.add_big("write", "Write speed", 1, 1)
        s.add_kv("cap", "Capacity")
        s.add_kv("fmt", "Formatted")
        s.add_kv("sys", "System disk")
        s.add_kv("swap", "Swap file")
        s.add_kv("type", "Type")
        s.set("cap", gb(self.info["capacity"]))
        s.set("type", self.info["type"])

    def _static(self):
        now = time.monotonic()
        if now - self._fmt_t < 30:
            return
        self._fmt_t = now
        total = 0
        system = False
        seen = set()
        for dev, mnt, _fs in _mounts():
            if _disk_of(dev) != self.dev or dev in seen:
                continue
            seen.add(dev)
            try:
                st = os.statvfs(mnt)
                total += st.f_blocks * st.f_frsize
            except OSError:
                pass
            if mnt == "/":
                system = True
        swap = False
        for line in tp.read_text("/proc/swaps").splitlines()[1:]:
            p = line.split()
            if not p:
                continue
            if p[1] == "partition" and _disk_of(p[0]) == self.dev:
                swap = True
            elif p[1] == "file" and system:
                # a swap file lives on the disk of its filesystem
                for dev, mnt, _fs in _mounts():
                    if p[0].startswith(mnt.rstrip("/") + "/") and _disk_of(dev) == self.dev:
                        swap = True
        s = self.stats
        s.set("fmt", gb(total) if total else ("—" if not self.dev.startswith("zram")
                                               else "Swap"))
        s.set("sys", "Yes" if system else "No")
        s.set("swap", "Yes" if swap else "No")

    def _now(self):
        return self.sys.disk_now.get(self.dev, (0.0, 0.0, 0.0, 0.0))

    def update_row(self):
        a = self._now()
        self.spark.set_series(self.sys.disk_hist.get(self.dev, []))
        self.row_val.set_label("%s\n%d%%" % (self.info["type"], round(a[0])))

    def update_view(self):
        a, rd, wr, resp = self._now()
        s = self.stats
        s.set("active", "%d%%" % round(a))
        s.set("resp", "%.1f ms" % resp)
        s.set("read", rate_bytes(rd))
        s.set("write", rate_bytes(wr))
        self.graph.set_series(self.sys.disk_hist.get(self.dev, []))
        r, w = self.sys.disk_rw_hist.get(self.dev, ([], []))
        self.rw.set_series(r, w)
        self.rw_max.set_label(rate_bytes(self.rw.current_max()))
        self._static()


class NetDevice(Device):
    kind = "net"
    graph_caption = "Throughput (receive solid, send dashed)"

    def __init__(self, page, index, iface, kind):
        self.iface = iface
        self.conn = kind
        super().__init__(page, "net%d" % index, kind)
        self._static_t = 0
        self.ssid = ""
        self.adapter = ""

    def build_body(self, v):
        self.graph = Graph(height=260, auto_floor=125000)
        self.graph.set_vexpand(True)
        self.top_r, self.bottom_l = self.graph_block(v, self.graph_caption, self.graph,
                                                     top_right="")

    def build_stats(self, s):
        base = "/sys/class/net/%s/device" % self.iface
        vendor = tp.read_text(base + "/vendor").strip().replace("0x", "")
        device = tp.read_text(base + "/device").strip().replace("0x", "")
        self.adapter = (tp.pci_name(vendor, device) if vendor and device else "") or self.iface
        self.model.set_label(self.adapter)
        s.add_big("send", "Send", 0, 0)
        s.add_big("recv", "Receive", 1, 0)
        s.add_kv("adapter", "Adapter name")
        if self.conn == "Wi-Fi":
            s.add_kv("ssid", "SSID")
        s.add_kv("type", "Connection type")
        s.add_kv("speed", "Link speed")
        s.add_kv("state", "State")
        s.add_kv("ipv4", "IPv4 address")
        s.add_kv("ipv6", "IPv6 address")
        s.add_kv("mac", "Hardware address")
        s.set("adapter", self.iface)
        s.set("type", self.conn)
        s.set("mac", tp.read_text("/sys/class/net/%s/address" % self.iface).strip())

    def _static(self):
        now = time.monotonic()
        if now - self._static_t < 10:
            return
        self._static_t = now
        s = self.stats
        s.set("ipv4", tp.iface_ipv4(self.iface) or "—")
        v6 = tp.iface_ipv6(self.iface)
        s.set("ipv6", v6[0] if v6 else "—")
        state = tp.read_text("/sys/class/net/%s/operstate" % self.iface).strip()
        s.set("state", {"up": "Connected", "down": "Disconnected",
                        "dormant": "Connecting"}.get(state, state or "—"))
        sp = tp.read_int("/sys/class/net/%s/speed" % self.iface)
        if sp and sp > 0:
            s.set("speed", "%s Mbps" % sp if sp < 1000 else "%g Gbps" % (sp / 1000))
        elif self.conn == "Wi-Fi":
            self._wifi_rate()
        if self.conn == "Wi-Fi" and have("nmcli"):
            run_cmd(["nmcli", "-t", "-f", "active,ssid", "dev", "wifi", "list", "ifname",
                     self.iface, "--rescan", "no"], self._ssid_done, mutating=False)

    def _wifi_rate(self):
        if not have("iw"):
            self.stats.set("speed", "—")
            return

        def done(ok, out, _err):
            m = re.search(r"tx bitrate:\s*([\d.]+)\s*MBit/s", out or "")
            self.stats.set("speed", "%s Mbps" % m.group(1) if m else "—")
        run_cmd(["iw", "dev", self.iface, "link"], done, mutating=False)

    def _ssid_done(self, ok, out, _err):
        ssid = ""
        for line in (out or "").splitlines():
            if line.startswith("yes:"):
                ssid = line[4:].replace("\\:", ":")
        self.ssid = ssid
        self.stats.set("ssid", ssid or "—")
        self.row_name.set_label("Wi-Fi")

    def _now(self):
        return self.sys.net_now.get(self.iface, (0.0, 0.0))

    def update_row(self):
        rx, tx = self._now()
        hist = self.sys.net_hist.get(self.iface, ([], []))
        self.spark.set_series(hist[0], hist[1])
        self.row_val.set_label("%s\nS: %s R: %s" % (self.ssid or self.iface,
                                                   rate_bits(tx).replace(" ", " "),
                                                   rate_bits(rx).replace(" ", " ")))

    def update_view(self):
        rx, tx = self._now()
        hist = self.sys.net_hist.get(self.iface, ([], []))
        self.graph.set_series(hist[0], hist[1])
        self.top_r.set_label(rate_bits(self.graph.current_max()))
        self.stats.set("send", rate_bits(tx))
        self.stats.set("recv", rate_bits(rx))
        self._static()


class GPUDevice(Device):
    kind = "gpu"

    def __init__(self, page, index, gpu):
        self.gpu = gpu
        super().__init__(page, "gpu%d" % index, "GPU %d" % index)

    def build_stats(self, s):
        self.model.set_label(self.gpu.name)
        s.add_big("util", "Utilization", 0, 0)
        s.add_big("freq", "Frequency", 1, 0)
        s.add_big("render", "3D / Render", 0, 1)
        s.add_big("video", "Video", 1, 1)
        s.add_big("copy", "Copy", 2, 1)
        s.add_kv("driver", "Driver")
        s.add_kv("pci", "PCI location")
        s.add_kv("maxfreq", "Max frequency")
        s.add_kv("vram", "Dedicated memory")
        s.add_kv("source", "Measured from")
        s.set("driver", self.gpu.driver or "—")
        s.set("pci", self.gpu.pci or "—")
        if self.gpu.has_busy:
            src = "gpu_busy_percent (whole GPU)"
        elif self.gpu.driver == "nvidia":
            src = "nvidia-smi" if have("nvidia-smi") else "Not available (nvidia-smi missing)"
        else:
            src = "Engine time of your apps"
        s.set("source", src)

    def update_row(self):
        v = self.sys.gpu_now.get(self.gpu.card, 0.0)
        self.spark.set_series(self.sys.gpu_hist.get(self.gpu.card, []))
        self.row_val.set_label("%s\n%d%%" % (self.gpu.name, round(v)))

    def update_view(self):
        s = self.stats
        card = self.gpu.card
        s.set("util", "%d%%" % round(self.sys.gpu_now.get(card, 0.0)))
        cur, mx = self.gpu.freq()
        nv = self.sys.nvidia_row(card)
        if nv:
            cur = nv.get("clock") or cur
        s.set("freq", "%d MHz" % cur if cur else "—")
        s.set("maxfreq", "%d MHz" % mx if mx else "—")
        eng = self.sys.gpu_engines.get(card, {})
        s.set("render", "%d%%" % round(eng.get("render", eng.get("gfx", 0.0))) if eng
              else "—")
        s.set("video", "%d%%" % round(max(eng.get("video", 0.0), eng.get("dec", 0.0),
                                          eng.get("enc", 0.0))) if eng else "—")
        s.set("copy", "%d%%" % round(eng.get("copy", 0.0)) if eng else "—")
        used, total = self.gpu.vram()
        if nv and nv.get("mem_total"):
            used, total = nv["mem_used"] * 1024 * 1024, nv["mem_total"] * 1024 * 1024
        s.set("vram", "%s / %s" % (gb(used), gb(total)) if total else "Shared with system")
        self.graph.set_series(self.sys.gpu_hist.get(card, []))


class PerformancePage(Page):
    key = "performance"
    title = "Performance"

    def __init__(self, win):
        super().__init__(win)
        self.devices = [CPUDevice(self), MemoryDevice(self), SwapDevice(self)]
        for i, d in enumerate(self.app.sys.disks):
            self.devices.append(DiskDevice(self, i, d))
        for i, (iface, kind) in enumerate(self.app.sys.nets):
            self.devices.append(NetDevice(self, i, iface, kind))
        for i, g in enumerate(self.app.sys.gpus):
            self.devices.append(GPUDevice(self, i, g))
        self.by_key = {d.key: d for d in self.devices}
        body = Gtk.Box(vexpand=True)
        self.list = Gtk.ListBox(css_classes=["hb-tm-devlist"],
                                selection_mode=Gtk.SelectionMode.SINGLE)
        for d in self.devices:
            self.list.append(d.row)
        self.list.connect("row-selected", self._row)
        ls = Gtk.ScrolledWindow(child=self.list, width_request=250, hexpand=False)
        ls.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        body.append(ls)
        body.append(Gtk.Box(css_classes=["hb-tm-divider-v"]))
        self.detail = Gtk.Stack(hexpand=True, vexpand=True)
        body.append(self.detail)
        self.box.append(body)
        self.current = None
        group = Gio.SimpleActionGroup()
        a = Gio.SimpleAction.new("copy", None)
        a.connect("activate", lambda *_: self._copy())
        group.add_action(a)
        cg = Gio.SimpleAction.new_stateful("cpu-graph", GLib.VariantType.new("s"),
                                           GLib.Variant.new_string(
                                               self.app.settings["cpu_graph"]))
        cg.connect("change-state", self._cpu_graph)
        group.add_action(cg)
        self.box.insert_action_group("perf", group)
        self.actions.append(tool_button("edit-copy-symbolic", "Copy", self._copy))
        self._pop = None

    def seconds_label(self):
        ms = self.app.settings.interval_ms() or 1000
        return "%d seconds" % round(60 * ms / 1000)

    def popup_menu(self, menu, parent, x, y):
        from gi.repository import Gdk
        if self._pop is not None:
            self._pop.unparent()
        pop = Gtk.PopoverMenu.new_from_model(menu)
        pop.set_parent(parent)
        pop.set_has_arrow(False)
        r = Gdk.Rectangle()
        r.x, r.y, r.width, r.height = int(x), int(y), 1, 1
        pop.set_pointing_to(r)
        self._pop = pop
        pop.popup()

    def _cpu_graph(self, action, value):
        action.set_state(value)
        self.app.settings["cpu_graph"] = value.get_string()
        self.by_key["cpu"].set_mode(value.get_string())
        self.on_tick()

    def _copy(self):
        if self.current is not None:
            copy_text(self.box, self.current.copy_text())
            self.win.notify_toast("Copied %s details" % self.current.name)

    def _row(self, _lb, row):
        if row is None:
            return
        d = self.by_key[row.key]
        if d.view is None:
            self.detail.add_named(d.build(), d.key)
        self.current = d
        self.detail.set_visible_child_name(d.key)
        d.update_row()
        d.update_view()

    def on_show(self, sub=None):
        key = sub or (self.current.key if self.current else "cpu")
        d = self.by_key.get(key) or self.devices[0]
        self.list.select_row(d.row)
        self.on_tick()

    def on_tick(self):
        for d in self.devices:
            d.update_row()
        if self.current is not None and self.current.view is not None:
            self.current.update_view()
            lbl = self.seconds_label()
            for attr in ("bottom_l", "bottom_l2"):
                w = getattr(self.current, attr, None)
                if w is not None:
                    w.set_label(lbl)


# ------------------------------------------------------------------ app history

class HistoryPage(Page):
    key = "history"
    title = "App history"

    def __init__(self, win):
        super().__init__(win)
        self.store = Gio.ListStore(item_type=Item)
        self.items = {}
        self.filter = Gtk.CustomFilter.new(self._match)
        self.query = ""
        filt = Gtk.FilterListModel(model=self.store, filter=self.filter)
        self.sort = Gtk.SortListModel(model=filt)
        sel = Gtk.SingleSelection(model=self.sort, autoselect=False, can_unselect=True)
        cols = [
            Col("name", "Name", lambda i: i.name, expand=True, kind="name",
                sort=key_sorter(lambda i: i.name.lower())),
            Col("cpu", "CPU time", lambda i: ts.format_cpu_time(i.cpu), width=130,
                kind="num", sort=key_sorter(lambda i: i.cpu),
                heat=lambda i: i.cpu / max(1.0, self._max_cpu)),
            Col("disk", "Disk", lambda i: "{:,.1f} MB".format(i.disk / 1048576), width=130,
                kind="num", sort=key_sorter(lambda i: i.disk),
                heat=lambda i: i.disk / max(1.0, self._max_disk)),
        ]
        self._max_cpu = 1.0
        self._max_disk = 1.0
        self.table = Table(sel, cols)
        self.sort.set_sorter(self.table.cv.get_sorter())
        self.since = label("", "hb-tm-sub", margin_start=20, margin_bottom=6)
        self.box.append(self.since)
        self.box.append(self.table.scroller)
        self.actions.append(tool_button("user-trash-symbolic", "Delete usage history",
                                        self._delete))
        self.table.cv.sort_by_column(self.table.columns["cpu"], Gtk.SortType.DESCENDING)
        self._t = 0

    def _match(self, item, *_):
        return not self.query or self.query in item.name.lower()

    def _delete(self):
        def go():
            self.app.history.clear()
            sync_store(self.store, [])
            self.items.clear()
            self.on_tick(force=True)
            self.win.notify_toast("Usage history deleted")
        confirm(self.win, "Delete usage history?",
                "The CPU time and disk use collected for each app will be reset.",
                "Delete", go)

    def on_search(self, text):
        self.query = (text or "").strip().lower()
        self.filter.changed(Gtk.FilterChange.DIFFERENT)

    def on_show(self, sub=None):
        self.on_tick(force=True)

    def on_tick(self, force=False):
        now = time.monotonic()
        if not force and now - self._t < 2:
            return
        self._t = now
        h = self.app.history
        since = GLib.DateTime.new_from_unix_local(int(h.since))
        self.since.set_label("Resource usage since %s for the current user account."
                             % since.format("%x"))
        rows = []
        for app_id, e in h.apps.items():
            it = self.items.get(app_id)
            if it is None:
                it = Item("app", app_id, e.get("name", app_id))
                icon = e.get("icon") or ""
                if icon:
                    try:
                        it.icon = Gio.Icon.new_for_string(icon)
                    except GLib.Error:
                        it.icon = None
                self.items[app_id] = it
            it.set_values(name=e.get("name", app_id), cpu=round(e.get("cpu", 0.0)),
                          disk=e.get("disk", 0))
            rows.append(it)
        self._max_cpu = max([i.cpu for i in rows] + [1.0])
        self._max_disk = max([i.disk for i in rows] + [1.0])
        sync_store(self.store, rows)
        col = self.table.cv.get_sorter().get_primary_sort_column()
        if col is not None:
            col.get_sorter().changed(Gtk.SorterChange.DIFFERENT)


# ------------------------------------------------------------------ startup apps

def autostart_dirs():
    user = os.environ.get("HARBOR_AUTOSTART_DIR") or os.path.join(
        GLib.get_user_config_dir(), "autostart")
    sysdirs = os.environ.get("HARBOR_AUTOSTART_SYSTEM_DIRS")
    if sysdirs:
        system = sysdirs.split(":")
    else:
        system = [os.path.join(d, "autostart") for d in GLib.get_system_config_dirs()]
    return user, system


def startup_impact(cpu_seconds, io_bytes):
    """Windows rule: High > 1 s CPU or > 3 MB disk, Medium > 0.3 s or 300 KB."""
    if cpu_seconds > 1.0 or io_bytes > 3 * 1024 * 1024:
        return "High"
    if cpu_seconds > 0.3 or io_bytes > 300 * 1024:
        return "Medium"
    return "Low"


class StartupPage(Page):
    key = "startup"
    title = "Startup apps"

    def __init__(self, win):
        super().__init__(win)
        self.store = Gio.ListStore(item_type=Item)
        self.query = ""
        self.filter = Gtk.CustomFilter.new(
            lambda i, *_: not self.query or self.query in i.name.lower()
            or self.query in i.entry.get("exec", "").lower())
        sort = Gtk.SortListModel(model=Gtk.FilterListModel(model=self.store, filter=self.filter))
        sel = Gtk.SingleSelection(model=sort, autoselect=False, can_unselect=True)
        cols = [
            Col("name", "Name", lambda i: i.name, expand=True, kind="name",
                sort=key_sorter(lambda i: i.name.lower())),
            Col("pub", "Publisher", lambda i: i.entry["publisher"], width=170,
                sort=key_sorter(lambda i: i.entry["publisher"].lower())),
            Col("status", "Status", lambda i: "Enabled" if i.entry["enabled"] else "Disabled",
                width=110, sort=key_sorter(lambda i: i.entry["enabled"])),
            Col("impact", "Startup impact", lambda i: i.status, width=140,
                dim=lambda i: i.status in ("Not measured", "None"),
                sort=key_sorter(lambda i: ["High", "Medium", "Low", "None",
                                           "Not measured"].index(i.status)
                                if i.status in ("High", "Medium", "Low", "None",
                                                "Not measured") else 9)),
        ]
        self.table = Table(sel, cols, on_menu=self._menu)
        sort.set_sorter(self.table.cv.get_sorter())
        self.table.cv.sort_by_column(self.table.columns["name"], Gtk.SortType.ASCENDING)
        sel.connect("notify::selected-item", lambda *_: self._sync())
        self.bios = label("", "hb-tm-sub", margin_start=20, margin_bottom=6)
        self.box.append(self.bios)
        self.box.append(self.table.scroller)
        self.toggle_btn = tool_button("media-playback-start-symbolic", "Enable", self._toggle)
        self.actions.append(self.toggle_btn)
        self.actions.append(tool_button("dialog-information-symbolic", "Properties",
                                        self._properties))
        self.actions.append(tool_button("view-refresh-symbolic", "Refresh", self.reload))
        group = Gio.SimpleActionGroup()
        for name, fn in (("toggle", self._toggle), ("location", self._location),
                         ("properties", self._properties)):
            a = Gio.SimpleAction.new(name, None)
            a.connect("activate", lambda *_a, f=fn: f())
            group.add_action(a)
        self.box.insert_action_group("st", group)
        self.loaded = False
        self.items = {}
        self._sync()

    def reload(self):
        user, system = autostart_dirs()
        desktops = [d for d in os.environ.get("XDG_CURRENT_DESKTOP", "GNOME").split(":") if d]
        entries = ts.list_autostart(user, system, desktops or ["GNOME"])
        rows = []
        for e in entries:
            it = self.items.get(e["id"])
            if it is None:
                it = Item("startup", e["id"], e["name"])
                self.items[e["id"]] = it
            it.entry = e
            icon = e.get("icon") or ""
            try:
                it.icon = Gio.Icon.new_for_string(icon) if icon else None
            except GLib.Error:
                it.icon = None
            it.icon_name = "application-x-executable-symbolic"
            it.set_values(name=e["name"], status=self._impact(e))
            it.emit("changed")
            rows.append(it)
        sync_store(self.store, rows)
        self.loaded = True
        self._sync()
        if have("systemd-analyze") and not getattr(self, "_bios_done", False):
            self._bios_done = True
            run_cmd(["systemd-analyze", "time"], self._analyze, mutating=False)

    def _analyze(self, ok, out, _err):
        m = re.search(r"([\d.]+)(m?s) \(firmware\)", out or "")
        total = re.search(r"= ([\dhmin.\s]+s)", out or "")
        parts = []
        if m:
            secs = float(m.group(1)) / (1000 if m.group(2) == "ms" else 1)
            parts.append("Last firmware (BIOS) time: %.1f seconds" % secs)
        if total:
            parts.append("Last boot: %s" % total.group(1).strip())
        self.bios.set_label("    ".join(parts) or "Startup apps run when you sign in.")

    def _impact(self, e):
        """Startup impact measured from the running processes of the entry's
        app (CPU time and disk I/O since they started)."""
        if not e["enabled"]:
            return "None"
        app_id = e["id"][:-8]
        cpu = 0.0
        io = 0
        found = False
        for p in self.app.proc_sampler.procs.values():
            if p.app is not None and p.app["id"] == app_id:
                found = True
                cpu += p.ticks / tp.CLK_TCK
                io += p.io_total or 0
        return startup_impact(cpu, io) if found else "Not measured"

    def on_show(self, sub=None):
        if not self.loaded:
            self.reload()
        if not self.bios.get_label():
            self.bios.set_label("Startup apps run when you sign in.")

    def on_search(self, text):
        self.query = (text or "").strip().lower()
        self.filter.changed(Gtk.FilterChange.DIFFERENT)

    def _selected(self):
        return self.table.selected_item()

    def _sync(self):
        it = self._selected()
        self.toggle_btn.set_sensitive(it is not None)
        if it is not None and it.entry["enabled"]:
            self.toggle_btn.text_label.set_label("Disable")
            self.toggle_btn.icon_image.set_from_icon_name("media-playback-stop-symbolic")
        else:
            self.toggle_btn.text_label.set_label("Enable")
            self.toggle_btn.icon_image.set_from_icon_name("media-playback-start-symbolic")

    def _toggle(self):
        it = self._selected()
        if it is None:
            return
        user, _system = autostart_dirs()
        want = not it.entry["enabled"]
        try:
            ts.set_autostart_enabled(it.entry, want, user)
        except OSError as e:
            self.win.notify_toast("Could not change %s: %s" % (it.name, e.strerror))
            return
        self.win.notify_toast("%s %s" % (it.name, "enabled" if want else "disabled"))
        self.reload()

    def _location(self):
        it = self._selected()
        if it is None:
            return
        d = os.path.dirname(it.entry["path"])
        spawn(["harbor-files", d] if have("harbor-files") else ["xdg-open", d])

    def _properties(self):
        it = self._selected()
        if it is None:
            return
        e = it.entry
        info_dialog(self.win, "Properties of %s" % e["name"], [
            ("Name", e["name"]), ("Description", e["comment"] or "—"),
            ("Command", e["exec"] or "—"), ("Publisher", e["publisher"]),
            ("Status", "Enabled" if e["enabled"] else "Disabled"),
            ("Startup impact", it.status), ("File", e["path"]),
            ("System file", e["system_path"] or "—"),
            ("User override", e["user_path"] or "—")])

    def _menu(self, cv, x, y):
        it = self._selected()
        if it is None:
            return
        m = Gio.Menu()
        m.append("Disable" if it.entry["enabled"] else "Enable", "st.toggle")
        m.append("Open file location", "st.location")
        m.append("Properties", "st.properties")
        self.table.popup(m, x, y)


# ------------------------------------------------------------------ services

class ServicesPage(Page):
    key = "services"
    title = "Services"

    def __init__(self, win):
        super().__init__(win)
        self.store = Gio.ListStore(item_type=Item)
        self.items = {}
        self.query = ""
        self.scope = "all"
        self.filter = Gtk.CustomFilter.new(self._match)
        sort = Gtk.SortListModel(model=Gtk.FilterListModel(model=self.store, filter=self.filter))
        sel = Gtk.SingleSelection(model=sort, autoselect=False, can_unselect=True)
        cols = [
            Col("name", "Name", lambda i: i.name, width=270, kind="text",
                sort=key_sorter(lambda i: i.name.lower())),
            Col("pid", "PID", lambda i: str(i.count) if i.count else "", width=80,
                kind="num", sort=key_sorter(lambda i: i.count)),
            Col("desc", "Description", lambda i: i.label, expand=True,
                sort=key_sorter(lambda i: i.label.lower())),
            Col("status", "Status", lambda i: i.status, width=100,
                dim=lambda i: i.status in ("Stopped", "Exited"),
                sort=key_sorter(lambda i: i.status)),
            Col("scope", "Scope", lambda i: "User" if i.scope == "user" else "System",
                width=90, sort=key_sorter(lambda i: i.scope)),
        ]
        self.table = Table(sel, cols, on_menu=self._menu)
        sort.set_sorter(self.table.cv.get_sorter())
        self.table.cv.sort_by_column(self.table.columns["name"], Gtk.SortType.ASCENDING)
        sel.connect("notify::selected-item", lambda *_: self._sync())
        self.box.append(self.table.scroller)
        a = self.actions
        self.scope_dd = dropdown(["All services", "System", "User"], 0, self._scope)
        self.scope_dd.set_margin_end(8)
        a.append(self.scope_dd)
        self.start_btn = tool_button("media-playback-start-symbolic", "Start",
                                     lambda: self._act("start"))
        self.stop_btn = tool_button("media-playback-stop-symbolic", "Stop",
                                    lambda: self._act("stop"))
        self.restart_btn = tool_button("view-refresh-symbolic", "Restart",
                                       lambda: self._act("restart"))
        for b in (self.start_btn, self.stop_btn, self.restart_btn):
            a.append(b)
        a.append(tool_button("view-refresh-symbolic", "Refresh", self.reload))
        group = Gio.SimpleActionGroup()
        for name in ("start", "stop", "restart"):
            act = Gio.SimpleAction.new(name, None)
            act.connect("activate", lambda *_a, n=name: self._act(n))
            group.add_action(act)
        g2 = Gio.SimpleAction.new("details", None)
        g2.connect("activate", lambda *_: self._details())
        group.add_action(g2)
        g3 = Gio.SimpleAction.new("copy", None)
        g3.connect("activate", lambda *_: self._copy())
        group.add_action(g3)
        self.box.insert_action_group("svc", group)
        self.loaded = False
        self.pending = 0
        self._rows = {}
        self._sync()

    def _match(self, item, *_):
        if self.scope != "all" and item.scope != self.scope:
            return False
        q = self.query
        return not q or q in item.name.lower() or q in item.label.lower() or \
            (q.isdigit() and str(item.count).startswith(q))

    def _scope(self, idx):
        self.scope = ["all", "system", "user"][idx]
        self.filter.changed(Gtk.FilterChange.DIFFERENT)

    def on_search(self, text):
        self.query = (text or "").strip().lower()
        self.filter.changed(Gtk.FilterChange.DIFFERENT)

    def on_show(self, sub=None):
        if not self.loaded:
            self.reload()

    def reload(self):
        self.loaded = True
        self._rows = {}
        self.pending = 2
        base = ["list-units", "--type=service", "--all", "--no-pager", "-o", "json"]
        run_cmd(["systemctl"] + base, lambda ok, out, err: self._listed("system", out),
                mutating=False)
        run_cmd(["systemctl", "--user"] + base,
                lambda ok, out, err: self._listed("user", out), mutating=False)

    def _listed(self, scope, out):
        rows = ts.parse_units_json(out, scope)
        self._rows[scope] = rows
        names = [r["name"] for r in rows if r["status"] in ("Running", "Starting",
                                                             "Stopping")]
        if names:
            argv = ["systemctl"] + (["--user"] if scope == "user" else []) + \
                ["show", "-p", "MainPID,Id", "--"] + names
            run_cmd(argv, lambda ok, o, e: self._shown(scope, o), mutating=False)
        else:
            self._shown(scope, "")

    def _shown(self, scope, out):
        pids = ts.parse_show(out)
        for r in self._rows.get(scope, []):
            r["pid"] = pids.get(r["name"], 0)
        self.pending -= 1
        self._apply(scope)

    def _apply(self, scope):
        rows = []
        for sc in ("system", "user"):
            for r in self._rows.get(sc, []):
                key = (sc, r["name"])
                it = self.items.get(key)
                if it is None:
                    it = Item("service", key, r["name"])
                    it.scope = sc
                    self.items[key] = it
                it.row = r
                it.set_values(label=r["description"], status=r["status"],
                              count=r.get("pid", 0) or 0)
                rows.append(it)
        if self.pending <= 0 or rows:
            sync_store(self.store, rows)
        self._sync()

    def _selected(self):
        return self.table.selected_item()

    def _sync(self):
        it = self._selected()
        running = it is not None and it.status in ("Running", "Starting")
        self.start_btn.set_sensitive(it is not None and not running)
        self.stop_btn.set_sensitive(it is not None and it.status in ("Running", "Starting",
                                                                     "Exited"))
        self.restart_btn.set_sensitive(it is not None)

    def _act(self, action):
        it = self._selected()
        if it is None:
            return
        argv = ts.service_argv(action, it.name, it.scope)

        def run():
            def done(ok, _out, err):
                if ok:
                    self.win.notify_toast("%s: %s" % (it.name, {"start": "started",
                                                                "stop": "stopped",
                                                                "restart": "restarted"}[action]))
                else:
                    self.win.notify_toast("Could not %s %s: %s" % (
                        action, it.name, (err or "").strip().splitlines()[-1:] and
                        (err or "").strip().splitlines()[-1] or "failed"))
                GLib.timeout_add(500, lambda: (self.reload(), False)[1])
            run_cmd(argv, done)

        if action in ("stop", "restart"):
            confirm(self.win, "%s %s?" % (action.capitalize(), it.name),
                    "Other services or apps that depend on %s may stop working%s." % (
                        it.label or it.name,
                        "" if it.scope == "user" else
                        ". You may be asked for an administrator password"),
                    action.capitalize(), run)
        else:
            run()

    def _details(self):
        it = self._selected()
        if it is None or not it.count:
            return
        self.win.show_page("details")
        self.win.pages["details"].select_pid(it.count)

    def _copy(self):
        it = self._selected()
        if it is not None:
            copy_text(self.box, it.name)

    def _menu(self, cv, x, y):
        it = self._selected()
        if it is None:
            return
        m = Gio.Menu()
        m.append("Start", "svc.start")
        m.append("Stop", "svc.stop")
        m.append("Restart", "svc.restart")
        s = Gio.Menu()
        if it.count:
            s.append("Go to details", "svc.details")
        s.append("Copy name", "svc.copy")
        m.append_section(None, s)
        self.table.popup(m, x, y)


# ------------------------------------------------------------------ settings

class SettingsPage(Page):
    key = "settings"
    title = "Settings"

    def __init__(self, win):
        super().__init__(win)
        sc, body = page_box(max_width=880, margin=20)
        body.set_margin_top(4)
        st = self.app.settings
        from .taskmgr import NAV
        self.page_keys = [k for k, _l, _i in NAV]
        names = [l for _k, l, _i in NAV]
        start = st["start_page"]
        dd_page = dropdown(names, self.page_keys.index(start) if start in self.page_keys else 0,
                           lambda i: self._set("start_page", self.page_keys[i]))
        speeds = [n for n, _ms in ts.INTERVALS]
        dd_speed = dropdown(["High (0.5 s)", "Normal (1 s)", "Low (4 s)", "Paused"],
                            speeds.index(st["interval"]) if st["interval"] in speeds else 1,
                            self._set_speed)
        body.append(section("General"))
        body.append(card_group(
            Card("Default start page", "The page Task Manager opens on",
                 icon="go-home-symbolic", suffix=dd_page),
            Card("Real time update speed", "How often the numbers and graphs refresh",
                 icon="power-profile-balanced-symbolic", suffix=dd_speed),
        ))
        body.append(section("Other options"))
        body.append(card_group(
            Card("Show full account name", "Users and Details show the full name",
                 icon="avatar-default-symbolic",
                 suffix=switch(bool(st["full_name"]), self._full_name)),
            Card("Group processes by type", "Apps, background and system processes",
                 icon="view-grid-symbolic",
                 suffix=switch(bool(st["group_by_type"]), self._group)),
            Card("App theme", "Follows the system light or dark style",
                 icon="display-brightness-symbolic"),
        ))
        body.append(section("Notifications"))
        self.notify_at = spin(70, 98, 1, int(st["notify_at"]),
                              lambda v: self._set("notify_at", v))
        self.notify_at.set_sensitive(bool(st["notify_memory"]))
        at = Gtk.Box(spacing=8, valign=Gtk.Align.CENTER)
        at.append(self.notify_at)
        at.append(label("%", "hb-tm-unit"))
        body.append(card_group(
            Card("Warn when memory use is high",
                 "A notification that opens Memory Sweep, at most every 10 minutes",
                 icon="dialog-warning-symbolic",
                 suffix=switch(bool(st["notify_memory"]), self._notify)),
            Card("Warn at", "Share of RAM in use", icon="org.gnome.SystemMonitor-symbolic",
                 suffix=at),
        ))
        from .taskmgr_sweep_page import MemoryOptimization
        self.optimization = MemoryOptimization(self)
        for w in self.optimization.widgets:
            body.append(w)
        body.append(section("About"))
        body.append(card_group(
            Card("About Task Manager", "Version, license and credits", icon=None,
                 on_activate=lambda: about_dialog(
                     self.win, "Harbor Task Manager", "io.github.hafij_deu.Harbor.TaskManager",
                     "Monitor processes, performance, startup apps and services.")),
        ))
        self.scroller = sc
        self.box.append(sc)
        self._opt_t = 0.0

    def on_show(self, sub=None):
        self.optimization.load(keep_form=self.optimization.form is not None)
        self._opt_t = time.monotonic()
        if sub == "optimization":
            scroll_to_widget(self.scroller, self.optimization.anchor)

    def on_tick(self):
        now = time.monotonic()
        if now - self._opt_t >= 5:
            self._opt_t = now
            self.optimization.on_tick()

    def _notify(self, on):
        self.app.settings["notify_memory"] = on
        self.notify_at.set_sensitive(on)

    def _set(self, key, value):
        self.app.settings[key] = value

    def _set_speed(self, idx):
        name = ts.INTERVALS[idx][0]
        self.app.settings["interval"] = name
        self.app.set_interval(self.app.settings.interval_ms())

    def _full_name(self, on):
        self.app.settings["full_name"] = on
        self.app.models.full_names = on
        self.app.refresh_now()

    def _group(self, on):
        act = self.win.lookup_action("group-by-type")
        if act is not None and act.get_state().get_boolean() != on:
            act.change_state(GLib.Variant.new_boolean(on))
