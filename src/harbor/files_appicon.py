"""App icon whose folder tab follows the accent colour.

data/app-icon-template.svg carries the token #HBACCENT in its tint filter
(tools/make_app_icon.py builds it). When Harbor's accent (or GNOME's) differs
from the shipped orange, a tinted copy goes to the user's hicolor theme, which
wins over /usr/share. Back on the default, that copy is removed again. Only a
file Harbor wrote itself (it carries MARK) is ever replaced or removed.
"""

import os

from gi.repository import GLib

APP_ID = "io.github.hafij_deu.Harbor.Files"
TOKEN = "#HBACCENT"
MARK = "<!-- harbor-accent-icon -->"
DATA = os.path.join(os.path.dirname(os.path.abspath(__file__)), "data")


def _default():
    try:
        with open(os.path.join(DATA, "app-icon-default.txt")) as f:
            return f.read().strip().lower()
    except OSError:
        return "#fb5b02"


def target():
    return os.path.join(GLib.get_user_data_dir(), "icons", "hicolor", "scalable",
                        "apps", APP_ID + ".svg")


def _ours(path):
    try:
        with open(path) as f:
            return MARK in f.read(200)
    except OSError:
        return False


def render(accent):
    """Tinted SVG text for accent "#rrggbb", or None when the template is missing."""
    try:
        with open(os.path.join(DATA, "app-icon-template.svg")) as f:
            svg = f.read()
    except OSError:
        return None
    return MARK + "\n" + svg.replace(TOKEN, accent)


def sync(accent):
    """Write or remove the user-level icon for accent ("#rrggbb" or None).

    Returns "written", "removed" or None (nothing to do). Never raises."""
    if os.environ.get("HB_NO_APPICON") or "PYTEST_CURRENT_TEST" in os.environ:
        return None
    try:
        path = target()
        accent = (accent or "").lower()
        if not accent or accent == _default():
            if os.path.exists(path) and _ours(path):
                os.remove(path)
                _touch(path)
                return "removed"
            return None
        svg = render(accent)
        if svg is None:
            return None
        if os.path.exists(path):
            if not _ours(path):
                return None              # someone else's icon: leave it alone
            with open(path) as f:
                if f.read() == svg:
                    return None
        os.makedirs(os.path.dirname(path), exist_ok=True)
        tmp = path + ".tmp"
        with open(tmp, "w") as f:
            f.write(svg)
        os.replace(tmp, path)
        _touch(path)
        return "written"
    except OSError:
        return None


def _touch(path):
    """Bump the theme dir mtime so GTK / the shell rescan it."""
    base = os.path.join(GLib.get_user_data_dir(), "icons", "hicolor")
    for d in (os.path.dirname(path), base):
        try:
            os.utime(d)
        except OSError:
            pass
