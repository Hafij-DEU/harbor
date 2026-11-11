"""History graphs for Harbor Task Manager, drawn with GSK paths (no cairo)."""

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gsk", "4.0")
gi.require_version("Graphene", "1.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, Graphene, Gsk, Gtk  # noqa: E402


def _rgba(r, g, b, a=1.0):
    c = Gdk.RGBA()
    c.red, c.green, c.blue, c.alpha = r, g, b, a
    return c


def accent_rgba():
    try:
        c = Adw.StyleManager.get_default().get_accent_color_rgba()
        if c is not None:
            return c
    except (AttributeError, TypeError):
        pass
    return _rgba(0.21, 0.52, 0.89)


def _with_alpha(c, a):
    return _rgba(c.red, c.green, c.blue, a)


def _rect(x, y, w, h):
    return Graphene.Rect().init(x, y, w, h)


def nice_max(v, floor=1.0):
    """Round a peak up to 1/2/5 x 10^n so the graph scale is readable."""
    v = max(v, floor)
    m = 1.0
    while m * 10 <= v:
        m *= 10
    for f in (1, 2, 5, 10):
        if v <= f * m:
            return f * m
    return 10 * m


class Graph(Gtk.Widget):
    """Scrolling history graph. series: list of value lists (newest last);
    the first series is filled, the others are drawn as dashed lines."""

    __gtype_name__ = "HarborTmGraph"

    def __init__(self, maximum=100.0, points=60, grid=True, width=-1, height=-1,
                 auto_floor=None):
        super().__init__()
        self.maximum = maximum
        self.auto_floor = auto_floor  # auto scale (rates) with this minimum
        self.points = points
        self.grid = grid
        self.series = [[]]
        self.set_size_request(width, height)
        self.set_overflow(Gtk.Overflow.HIDDEN)

    def set_series(self, *series):
        self.series = [list(s) for s in series]
        self.queue_draw()

    def current_max(self):
        if self.auto_floor is None:
            return self.maximum
        peak = max((max(s) for s in self.series if s), default=0.0)
        return nice_max(peak * 1.1, self.auto_floor)

    def do_snapshot(self, snap):
        w = self.get_width()
        h = self.get_height()
        if w <= 2 or h <= 2:
            return
        fg = self.get_color()
        acc = accent_rgba()
        snap.append_color(_with_alpha(acc, 0.04), _rect(0, 0, w, h))
        if self.grid:
            gc = _with_alpha(fg, 0.07)
            for i in range(1, 10):
                y = round(h * i / 10)
                snap.append_color(gc, _rect(0, y, w, 1))
            for i in range(1, 20):
                x = round(w * i / 20)
                snap.append_color(gc, _rect(x, 0, 1, h))
        border = _with_alpha(acc, 0.75)
        snap.append_color(border, _rect(0, 0, w, 1))
        snap.append_color(border, _rect(0, h - 1, w, 1))
        snap.append_color(border, _rect(0, 0, 1, h))
        snap.append_color(border, _rect(w - 1, 0, 1, h))
        mx = self.current_max() or 1.0
        step = (w - 1) / max(1, self.points - 1)
        for idx, values in enumerate(self.series):
            vals = values[-self.points:]
            if len(vals) < 2:
                continue
            x0 = (w - 1) - step * (len(vals) - 1)
            pts = [(x0 + i * step, (h - 1) - (h - 2) * min(1.0, max(0.0, v / mx)))
                   for i, v in enumerate(vals)]
            line = Gsk.PathBuilder.new()
            line.move_to(*pts[0])
            for p in pts[1:]:
                line.line_to(*p)
            if idx == 0:
                fill = Gsk.PathBuilder.new()
                fill.move_to(pts[0][0], h - 1)
                for p in pts:
                    fill.line_to(*p)
                fill.line_to(pts[-1][0], h - 1)
                fill.close()
                snap.append_fill(fill.to_path(), Gsk.FillRule.WINDING, _with_alpha(acc, 0.18))
            stroke = Gsk.Stroke.new(1.4 if self.grid else 1.0)
            if idx > 0:
                stroke.set_dash([4.0, 3.0])
            snap.append_stroke(line.to_path(), stroke, acc)


class CompositionBar(Gtk.Widget):
    """Memory composition: In use | Cached (standby) | Free."""

    __gtype_name__ = "HarborTmCompositionBar"

    def __init__(self, height=44):
        super().__init__()
        self.parts = (0.0, 0.0)
        self.set_size_request(-1, height)
        self.set_has_tooltip(True)

    def set_parts(self, used, cached):
        self.parts = (max(0.0, used), max(0.0, cached))
        self.queue_draw()

    def do_snapshot(self, snap):
        w = self.get_width()
        h = self.get_height()
        if w <= 2:
            return
        acc = accent_rgba()
        used, cached = self.parts
        uw = round(w * min(1.0, used))
        cw = round(w * min(1.0 - min(1.0, used), cached))
        snap.append_color(_with_alpha(acc, 0.04), _rect(0, 0, w, h))
        snap.append_color(_with_alpha(acc, 0.45), _rect(0, 0, uw, h))
        if cw > 0:
            snap.append_color(_with_alpha(acc, 0.16), _rect(uw, 0, cw, h))
        sep = _with_alpha(acc, 0.85)
        for x in (uw, uw + cw):
            if 0 < x < w:
                snap.append_color(sep, _rect(x, 0, 1, h))
        border = _with_alpha(acc, 0.75)
        snap.append_color(border, _rect(0, 0, w, 1))
        snap.append_color(border, _rect(0, h - 1, w, 1))
        snap.append_color(border, _rect(0, 0, 1, h))
        snap.append_color(border, _rect(w - 1, 0, 1, h))
