"""Start a Harbor app, run --do steps, render its window to PNG (see shot.sh)."""

import argparse
import importlib
import os
import sys
import traceback

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "src"))

import gi  # noqa: E402

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
gi.require_version("Gsk", "4.0")
gi.require_version("Graphene", "1.0")
from gi.repository import Adw, GLib, Graphene, Gsk, Gtk  # noqa: E402

p = argparse.ArgumentParser()
p.add_argument("app")
p.add_argument("out")
p.add_argument("--size", default="1200x780")
p.add_argument("--theme", default="dark", choices=["dark", "light"])
p.add_argument("--do", action="append", default=[])
p.add_argument("--delay", type=int, default=500)
a = p.parse_args()
W, H = (int(v) for v in a.size.split("x"))

mod = importlib.import_module("harbor." + a.app)
application = mod.App()
state = {"rc": 1}


def fail(msg):
    print("drive.py:", msg, file=sys.stderr)
    application.quit()
    return False


def render(win):
    w, h = win.get_width(), win.get_height()
    paintable = Gtk.WidgetPaintable.new(win)
    snap = Gtk.Snapshot()
    paintable.snapshot(snap, w, h)
    node = snap.to_node()
    if node is None:
        return fail("no render node (layout not ready)")
    renderer = Gsk.CairoRenderer()
    renderer.realize_for_display(win.get_display())
    rect = Graphene.Rect().init(0, 0, w, h)
    tex = renderer.render_texture(node, rect)
    tex.save_to_png(a.out)
    renderer.unrealize()
    print("wrote", a.out, "%dx%d" % (w, h))
    state["rc"] = 0
    application.quit()
    return False


def start():
    Adw.StyleManager.get_default().set_color_scheme(
        Adw.ColorScheme.FORCE_DARK if a.theme == "dark" else Adw.ColorScheme.FORCE_LIGHT)
    win = application.get_active_window()
    if win is None:
        return fail("app has no window")
    win.set_default_size(W, H)
    steps = list(a.do)
    env = {"win": win, "app": application, "Gtk": Gtk, "GLib": GLib, "Adw": Adw}

    def step():
        if steps:
            code = steps.pop(0)
            try:
                exec(code, env)
            except Exception:
                traceback.print_exc()
                return fail("step failed: " + code)
            return True
        # one more tick so the last change is laid out before rendering
        GLib.timeout_add(a.delay, lambda: render(win))
        return False
    GLib.timeout_add(a.delay, step)
    return False


GLib.timeout_add(900, start)
application.run([sys.argv[0]])
sys.exit(state["rc"])
