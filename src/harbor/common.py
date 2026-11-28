"""Shared look and widgets for the Harbor apps.

Every Harbor app imports this module. It holds the Windows 11-style theme
(light and dark, accent taken from the GNOME accent colour), the window
shell, the left navigation list, setting cards and small helpers.
"""

import os
import shlex
import string
import sys

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, Gio, GLib, GObject, Gtk  # noqa: E402

from . import VERSION  # noqa: E402

# HARBOR_DRY_RUN=1: never run commands that change the system. run_cmd() and
# spawn() only log them to stderr (and to DRY_RUN_LOG for tests).
DRY_RUN = os.environ.get("HARBOR_DRY_RUN") == "1"
DRY_RUN_LOG = []

# ------------------------------------------------------------------ theme

PALETTES = {
    "dark": dict(
        window="#202020", sidebar="#202020", surface="#272727", card="#2b2b2b",
        card_hover="#323232", card_active="#282828", border="rgba(255,255,255,0.07)",
        divider="rgba(255,255,255,0.08)", text="#ffffff", dim="rgba(255,255,255,0.62)",
        faint="rgba(255,255,255,0.40)", nav_hover="rgba(255,255,255,0.06)",
        nav_sel="rgba(255,255,255,0.09)", entry="#2d2d2d", entry_border="rgba(255,255,255,0.10)",
        shadow="rgba(0,0,0,0.35)", heat1="rgba(255,196,0,0.10)", heat2="rgba(255,150,0,0.22)",
        heat3="rgba(255,90,40,0.34)",
    ),
    "light": dict(
        window="#f3f3f3", sidebar="#f3f3f3", surface="#f9f9f9", card="#fbfbfb",
        card_hover="#f6f6f6", card_active="#f0f0f0", border="rgba(0,0,0,0.07)",
        divider="rgba(0,0,0,0.08)", text="#1a1a1a", dim="rgba(0,0,0,0.60)",
        faint="rgba(0,0,0,0.42)", nav_hover="rgba(0,0,0,0.04)",
        nav_sel="rgba(0,0,0,0.06)", entry="#ffffff", entry_border="rgba(0,0,0,0.10)",
        shadow="rgba(0,0,0,0.10)", heat1="rgba(255,210,0,0.16)", heat2="rgba(255,170,0,0.28)",
        heat3="rgba(255,110,40,0.36)",
    ),
    # "System" look (Harbor Files > Options > Style): every colour comes from
    # the libadwaita / system theme, Harbor keeps only its layout.
    "system": dict(
        window="var(--window-bg-color)", sidebar="var(--sidebar-bg-color)",
        surface="var(--view-bg-color)", card="var(--card-bg-color)",
        card_hover="alpha(currentColor,0.05)", card_active="alpha(currentColor,0.08)",
        border="var(--border-color)", divider="var(--border-color)",
        text="var(--window-fg-color)", dim="alpha(currentColor,0.62)",
        faint="alpha(currentColor,0.42)", nav_hover="alpha(currentColor,0.07)",
        nav_sel="alpha(currentColor,0.10)", entry="var(--view-bg-color)",
        entry_border="var(--border-color)", shadow="alpha(black,0.15)",
        heat1="rgba(255,210,0,0.16)", heat2="rgba(255,170,0,0.28)",
        heat3="rgba(255,110,40,0.36)",
    ),
}

CSS = string.Template("""
window.hb-window, window.hb-window > contents { background: $window; color: $text; }
.hb-window headerbar { background: $window; box-shadow: none; border: none; min-height: 40px; }
.hb-window headerbar .title { font-weight: normal; font-size: 12px; }
.hb-sidebar { background: $sidebar; }
.hb-content { background: $window; }
.hb-surface { background: $surface; border-top-left-radius: 8px; border: 1px solid $border; }

/* navigation list (Settings, Task Manager, Files sidebar) */
list.hb-nav, list.hb-nav > row { background: transparent; }
list.hb-nav > row { border-radius: 5px; padding: 0; margin: 1px 6px; min-height: 36px; }
list.hb-nav > row:hover { background: $nav_hover; }
list.hb-nav > row:selected, list.hb-nav > row:selected:hover { background: $nav_sel; color: $text; }
list.hb-nav > row:selected .hb-nav-pill { background: var(--accent-bg-color); }
.hb-nav-pill { min-width: 3px; min-height: 16px; border-radius: 2px; background: transparent; }
.hb-nav-label { font-size: 14px; }
.hb-nav-heading { font-size: 12px; color: $dim; margin: 10px 18px 2px 18px; }

/* icon badge: solid accent square with a white symbolic icon */
.hb-badge { background: var(--accent-bg-color); color: white; border-radius: 6px;
            min-width: 24px; min-height: 24px; }
.hb-badge.hb-badge-big { border-radius: 9px; min-width: 36px; min-height: 36px; }

/* text */
.hb-title { font-size: 28px; font-weight: 600; }
.hb-subtitle { font-size: 20px; font-weight: 600; }
.hb-section { font-size: 14px; font-weight: 600; margin-top: 14px; margin-bottom: 4px; }
.hb-dim { color: $dim; }
.hb-faint { color: $faint; }
.hb-small { font-size: 12px; }
.hb-crumb { font-size: 28px; font-weight: 600; color: $dim; }
.hb-crumb-last { font-size: 28px; font-weight: 600; color: $text; }
.hb-accent-text { color: var(--accent-color); }

/* setting cards */
.hb-card { background: $card; border: 1px solid $border; border-radius: 6px;
           padding: 9px 16px; min-height: 42px; color: $text; }
button.hb-card { font-weight: normal; box-shadow: none; }
button.hb-card:hover { background: $card_hover; }
button.hb-card:active { background: $card_active; }
.hb-card-title { font-size: 14px; }
.hb-card-sub { font-size: 12px; color: $dim; }
.hb-card-group { border-radius: 6px; }
.hb-tile { background: $card; border: 1px solid $border; border-radius: 8px; padding: 12px; }
button.hb-tile:hover { background: $card_hover; }

/* entries and buttons */
entry.hb-search, searchentry.hb-search { background: $entry; border: 1px solid $entry_border;
    border-bottom: 1px solid $faint; border-radius: 5px; min-height: 32px; }
button.hb-accent { background: var(--accent-bg-color); color: var(--accent-fg-color);
    border-radius: 5px; padding: 5px 16px; font-weight: normal; }
button.hb-accent:hover { filter: brightness(1.08); }
button.hb-plain { background: $card; border: 1px solid $border; border-radius: 5px;
    padding: 5px 14px; font-weight: normal; }
button.hb-plain:hover { background: $card_hover; }
button.hb-link { color: var(--accent-color); background: none; padding: 0 2px; font-weight: normal; }
button.hb-tool { background: none; border-radius: 5px; padding: 4px 8px; font-weight: normal; min-height: 28px; }
button.hb-tool:hover { background: $nav_hover; }
button.hb-tool:disabled { opacity: 0.4; }
.hb-divider { background: $divider; min-width: 1px; min-height: 1px; }

/* table (Task Manager, Files details) */
.hb-table-head { font-size: 12px; color: $dim; }
.hb-table-head-big { font-size: 18px; color: $text; font-weight: 600; }
.hb-heat1 { background: $heat1; }
.hb-heat2 { background: $heat2; }
.hb-heat3 { background: $heat3; }
columnview.hb-table, columnview.hb-table > listview { background: transparent; }
columnview.hb-table > listview > row { border-radius: 0; }
columnview.hb-table > listview > row:hover { background: $nav_hover; }
columnview.hb-table > listview > row:selected { background: $nav_sel; color: $text; }
columnview.hb-table > header > button { background: transparent; border: none; box-shadow: none; }
""")


# ------------------------------------------------------------------ glass

# Glass shared by every Harbor app and Fusion Desk. The values are Fusion
# Desk's glass-* settings. Harbor Settings > Personalisation writes them to
# ~/.config/harbor/: "glass" (all Harbor apps, or "global" = follow Fusion
# Desk), "glass-files" / "glass-taskmgr" / "glass-settings" (one app, or
# "harbor" = follow all Harbor apps) and "glass-global" (a copy of the
# desktop values, used when Fusion Desk is not installed).
FUSION_SCHEMA = "org.gnome.shell.extensions.fusion-desk"
FUSION_UUID = "fusion-desk@hafij-deu.github.io"
GLASS_STRENGTHS = ("style-strength", "shine-strength", "rim-strength", "glare-strength",
                   "shadow-strength", "light-strength")   # percent, 0-200, 100 = normal
GLASS_KEYS = ("effect", "material", "style", "opacity", "rim", "glare", "shadow",
              "light-follows") + GLASS_STRENGTHS
GLASS_EFFECTS = ("OFF", "SOFT", "STRONG")
GLASS_MATERIALS = ("GLASS", "GLOSS")
GLASS_STYLES = ("CLEAR", "FROSTED", "GEL", "TINTED", "SMOKE", "HALO", "GLOW", "PLANNER")
# PLANNER = the Daily Routine Planner look, palette v3 ("Planner Glass"): one
# violet-indigo hue family. An aurora backdrop (pink -> violet -> sky) (dark: ink
# indigo) with real glass cards on it: translucent white (dark: smoky violet)
# with a top-to-bottom sheen, a white rim, a bright top edge and a soft
# coloured shadow. Header = near-solid bar; footer = floating glass card.
# Spec: ~/Music/apps/glass-theme/PLANNER-SPEC.md
PLANNER = {
    False: {"stops": ((255, 214, 236), (226, 214, 255), (200, 226, 255)),
            "card": "rgba(255,255,255,0.30)",
            "card_img": "linear-gradient(to bottom, rgba(255,255,255,0.72), rgba(255,255,255,0.40))",
            "surface": "rgba(255,255,255,0.0)", "entry": "rgba(255,255,255,0.55)",
            "window": "rgba(255,255,255,0.10)", "sidebar": "rgba(255,255,255,0.30)",
            "nav_hover": "rgba(255,255,255,0.45)", "rim": "rgba(255,255,255,0.85)",
            "spec": "inset 0 1px rgba(255,255,255,0.95)", "line": "rgba(255,255,255,0.90)",
            "text": "#1e1b3a", "soft": "#5b5778", "accent_text": "#4f46e5",
            "btn": "rgba(255,255,255,0.50)", "btn_hover": "rgba(255,255,255,0.80)",
            "btn_text": "#4338ca", "btn_line": "rgba(255,255,255,0.90)",
            "bar": "rgba(255,255,255,0.90)", "hair": "rgba(99,102,241,0.10)", "shade": "76,61,180"},
    True: {"stops": ((48, 20, 60), (30, 24, 74), (14, 34, 70)),
           "card": "rgba(40,35,76,0.55)",
           "card_img": "linear-gradient(to bottom, rgba(255,255,255,0.10), rgba(255,255,255,0.03))",
           "surface": "rgba(20,17,40,0.25)", "entry": "rgba(255,255,255,0.06)",
           "window": "rgba(20,17,40,0.20)", "sidebar": "rgba(20,17,40,0.40)",
           "nav_hover": "rgba(255,255,255,0.06)", "rim": "rgba(255,255,255,0.13)",
           "spec": "inset 0 1px rgba(255,255,255,0.16)", "line": "rgba(255,255,255,0.14)",
           "text": "#eeedf9", "soft": "#aaa7c8", "accent_text": "#a5b4fc",
           "btn": "rgba(255,255,255,0.08)", "btn_hover": "rgba(255,255,255,0.14)",
           "btn_text": "#d4d0ff", "btn_line": "rgba(255,255,255,0.14)",
           "bar": "rgba(24,21,48,0.92)", "hair": "rgba(255,255,255,0.07)", "shade": "0,0,0"},
}
PLANNER_ACCENT = "linear-gradient(135deg, #6366f1, #8b5cf6)"
PLANNER_SHADOW_ACCENT = "0 8px 18px -6px rgba(99,102,241,0.50)"


def _planner_backdrop(dark, alpha=1.0):
    return "linear-gradient(150deg, %s)" % ", ".join(
        "rgba(%d,%d,%d,%.2f)" % (*c, alpha) for c in PLANNER[bool(dark)]["stops"])


GLASS_DEFAULT = {"effect": "STRONG", "material": "GLASS", "style": "FROSTED", "opacity": 75,
                 "rim": True, "glare": True, "shadow": False, "light-follows": False,
                 **{k: 100 for k in GLASS_STRENGTHS}}
GLASS_OFF = dict(GLASS_DEFAULT, effect="OFF", opacity=100)
# An app's own quick choice (Files > Options, Task Manager > Settings);
# "custom" means Harbor Settings gave it its own full set of values.
GLASS_APP_CHOICES = ("harbor", "off", "soft", "strong", "planner", "custom")
GLASS_APP_LABELS = (("harbor", "Same as all Harbor apps"), ("off", "Off"), ("soft", "Soft"),
                    ("strong", "Strong"), ("planner", "Lumina Glass"),
                    ("custom", "Custom (Harbor Settings)"))


def harbor_config_path(name):
    base = os.environ.get("XDG_CONFIG_HOME") or os.path.join(os.path.expanduser("~"), ".config")
    return os.path.join(base, "harbor", name)


def _read_config(name):
    try:
        with open(harbor_config_path(name), encoding="utf-8") as fh:
            return fh.read().strip()
    except OSError:
        return None


def _write_config(name, text):
    path = harbor_config_path(name)
    try:
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(text)
    except OSError:
        pass


def glass_set(spec, key, value):
    """Set one glass value from text ('STRONG', '75', 'true'); bad values are ignored."""
    key = key.strip().removeprefix("glass-")
    value = str(value).strip().strip("'")
    if key == "effect" and value.upper() in GLASS_EFFECTS:
        spec[key] = value.upper()
    elif key == "material" and value.upper() in GLASS_MATERIALS:
        spec[key] = value.upper()
    elif key == "style" and value.upper() in GLASS_STYLES:
        spec[key] = value.upper()
    elif key == "opacity" and value.isdigit():
        spec[key] = max(10, min(100, int(value)))
    elif key in GLASS_STRENGTHS and value.isdigit():
        spec[key] = min(200, int(value))
    elif key in ("rim", "glare", "shadow", "light-follows"):
        spec[key] = value == "true"


def parse_glass(text):
    """`key=value` pairs (lines or commas) into a glass dict. The old one-word
    styles off / glossy / liquid still work."""
    text = (text or "").strip()
    if text == "off":
        return dict(GLASS_OFF)
    if text == "glossy":
        return dict(GLASS_DEFAULT, effect="SOFT", material="GLOSS", style="GEL", opacity=100)
    if text == "liquid":
        return dict(GLASS_DEFAULT, style="CLEAR", opacity=80)
    spec = dict(GLASS_DEFAULT)
    for pair in text.replace(",", "\n").splitlines():
        if "=" in pair:
            glass_set(spec, *pair.split("=", 1))
    return spec


def glass_text(spec):
    def show(v):
        return ("true" if v else "false") if isinstance(v, bool) else str(v)
    return "".join(f"{k}={show(spec[k])}\n" for k in GLASS_KEYS)


def fusion_settings():
    """Fusion Desk's Gio.Settings, or None when it is not installed."""
    for base in (os.path.join(GLib.get_user_data_dir(), "gnome-shell", "extensions"),
                 "/usr/share/gnome-shell/extensions", "/usr/local/share/gnome-shell/extensions"):
        folder = os.path.join(base, FUSION_UUID, "schemas")
        if not os.path.exists(os.path.join(folder, "gschemas.compiled")):
            continue
        try:
            source = Gio.SettingsSchemaSource.new_from_directory(
                folder, Gio.SettingsSchemaSource.get_default(), False)
            schema = source.lookup(FUSION_SCHEMA, False)
        except GLib.Error:
            continue
        if schema is not None:
            return Gio.Settings.new_full(schema, None, None)
    return None


def fusion_blurs_windows(fusion=None):
    """Whether Fusion Desk blurs the desktop behind Harbor windows.

    Without that blur a see-through window shows sharp windows behind it, so
    Harbor then keeps the window solid. HARBOR_GLASS_BLUR=0|1 overrides.
    """
    env = os.environ.get("HARBOR_GLASS_BLUR")
    if env is not None:
        return env != "0"
    fusion = fusion or fusion_settings()
    if fusion is None or "harbor-glass-blur" not in fusion.props.settings_schema.list_keys():
        return False
    if not fusion.get_boolean("harbor-glass-blur"):
        return False
    try:
        shell = Gio.Settings(schema_id="org.gnome.shell")
        return (FUSION_UUID in shell.get_strv("enabled-extensions")
                and FUSION_UUID not in shell.get_strv("disabled-extensions")
                and not shell.get_boolean("disable-user-extensions"))
    except Exception:
        return False


def global_glass(fusion=None):
    """The desktop's glass: Fusion Desk's settings, or Harbor's copy of them."""
    fusion = fusion or fusion_settings()
    if fusion is not None:
        spec = dict(GLASS_DEFAULT)
        keys = fusion.props.settings_schema.list_keys()
        for key in GLASS_KEYS:
            if "glass-" + key in keys:
                glass_set(spec, key, fusion.get_value("glass-" + key).print_(False))
        return spec
    text = _read_config("glass-global")
    return parse_glass(text) if text else dict(GLASS_DEFAULT)


def scope_glass(scope):
    """A scope's own values ("harbor" or an app name), or None to follow."""
    text = _read_config("glass" if scope == "harbor" else "glass-" + scope)
    if text is None:
        return dict(GLASS_OFF) if scope == "harbor" else None
    if text in ("", "global", "harbor"):
        return None
    return parse_glass(text)


def resolve_glass(app, fusion=None):
    """The glass `app` paints with, following its scope chain."""
    spec = scope_glass(app)
    if spec is None:
        spec = scope_glass("harbor")
    if spec is None:
        spec = global_glass(fusion)
    return spec


def app_glass_choice(app):
    """The quick choice that matches the app's own glass file."""
    spec = scope_glass(app)
    if spec is None:
        return "harbor"
    base = resolve_glass("harbor") if scope_glass("harbor") is not None else global_glass()
    if spec["effect"] == "OFF":
        return "off"
    if spec["style"] == "PLANNER" and base["style"] != "PLANNER":
        return "planner"
    if dict(spec, effect=base["effect"]) == base:
        return spec["effect"].lower()
    return "custom"


def set_app_glass_choice(app, choice):
    """Write an app's quick choice; "custom" leaves its file alone."""
    if choice == "harbor":
        _write_config("glass-" + app, "harbor")
    elif choice in ("off", "soft", "strong"):
        base = resolve_glass("harbor") if scope_glass("harbor") is not None else global_glass()
        _write_config("glass-" + app, glass_text(dict(base, effect=choice.upper())))
    elif choice == "planner":   # the Daily Routine Planner glass, this app only
        _write_config("glass-" + app, glass_text(dict(
            GLASS_DEFAULT, effect="STRONG", style="PLANNER", opacity=100, glare=False, shadow=True)))


def _rgb(value):
    if isinstance(value, str) and value.startswith("#") and len(value) == 7:
        return tuple(int(value[i:i + 2], 16) for i in (1, 3, 5))
    return None


def _alpha(value, alpha):
    rgb = _rgb(value)
    if rgb:
        return "rgba(%d,%d,%d,%.3f)" % (*rgb, alpha)
    if value.startswith("var(") or value.startswith("alpha("):
        return f"alpha({value},{alpha:.3f})"
    return value


def _mix(value, other, t):
    rgb = _rgb(value)
    if not rgb:
        return value
    return "#%02x%02x%02x" % tuple(round(a + (b - a) * t) for a, b in zip(rgb, other))


def white(a):
    return "rgba(255,255,255,%.3f)" % max(0.0, min(1.0, a))


def black(a):
    return "rgba(0,0,0,%.3f)" % max(0.0, min(1.0, a))


def accent(a):
    return "alpha(var(--accent-bg-color),%.3f)" % max(0.0, min(1.0, a))


def glass_palette(pal, spec, dark, see_through=None):
    """Palette colours for a glass spec: the style's tint and the opacity."""
    if spec["effect"] == "OFF":
        return pal
    pal = dict(pal)
    style = spec["style"]
    if style == "PLANNER":
        return _planner_palette(pal, spec, dark, see_through)
    a = spec["opacity"] / 100
    if style == "CLEAR":
        a *= 0.7
    if spec["effect"] == "SOFT":
        a = 0.5 + a * 0.5
    if style == "GLOW":
        a = min(a, 0.6)   # the coloured light behind must show through
    a = max(0.2, a)

    def tint(c):
        if style == "FROSTED":
            return _mix(c, (255, 255, 255), 0.06 if dark else 0.30)
        if style == "SMOKE":
            return _mix(c, (0, 0, 0), 0.30 if dark else 0.10)
        if style == "GLOW" and dark:
            return _mix(c, (44, 46, 50), 0.6)
        return c
    for key, extra in (("card", 0), ("card_hover", 0.12), ("card_active", 0.08),
                       ("entry", 0.10), ("surface", 0)):
        pal[key] = _alpha(tint(pal[key]), min(1.0, a + extra))
    if spec["rim"]:
        pal["border"] = white((0.13 if dark else 0.75) * (1 if spec["effect"] == "STRONG" else 0.7))
    if see_through is None:
        see_through = spec["opacity"] < 100
    if see_through:
        page = max(0.35, spec["opacity"] / 100)
        pal["window"] = _alpha(pal["window"], page)
        pal["sidebar"] = _alpha(pal["sidebar"], page * 0.8)
    return pal


def _planner_palette(pal, spec, dark, see_through):
    """PLANNER colours. The window paints the planner gradient (see
    _planner_css); window/sidebar are thin veils, cards are glass on it."""
    p = PLANNER[bool(dark)]
    pal["card"] = p["card"]
    pal["card_hover"] = p["btn_hover"]
    pal["card_active"] = p["btn"]
    pal["surface"] = p["surface"]
    pal["entry"] = p["entry"]
    pal["entry_border"] = p["line"]
    pal["window"] = p["window"]
    pal["sidebar"] = p["sidebar"]
    pal["nav_hover"] = p["nav_hover"]
    pal["nav_sel"] = "rgba(99,102,241,%.2f)" % (0.24 if dark else 0.14)
    pal["border"] = p["rim"]
    return pal


def _planner_css(spec, dark, see_through):
    """PLANNER shapes and colours: the planner gradient, glass cards (sheen,
    white rim, bright top edge, coloured shadow), planner corner scale,
    indigo accent, a near-solid header and a floating glass footer card."""
    p = PLANNER[bool(dark)]
    backdrop = _planner_backdrop(dark, max(0.35, spec["opacity"] / 100) if see_through else 1.0)
    ds = spec["shadow-strength"] / 100
    k = 1.8 if dark else 1.0
    card_shadow = (f"0 10px 24px -12px rgba({p['shade']},{min(1, 0.30 * ds * k):.2f}), "
                   f"0 2px 6px -2px rgba({p['shade']},{min(1, 0.10 * ds * k):.2f})")
    big_shadow = f"0 16px 32px -14px rgba({p['shade']},{min(1, 0.38 * ds * k):.2f})"
    glass = (f"background-color: {p['card']}; background-image: {p['card_img']}; "
             f"border: 1px solid {p['rim']};")
    return f"""
:root {{ --accent-bg-color: #6366f1; --accent-fg-color: #ffffff; --accent-color: {p["accent_text"]};
    --window-fg-color: {p["text"]}; --view-fg-color: {p["text"]}; }}
window.hb-window {{ background-image: {backdrop}; color: {p["text"]}; }}
window.hb-window .dim-label {{ color: {p["soft"]}; }}
window.hb-window headerbar {{ background-color: {p["bar"]}; color: {p["text"]};
    box-shadow: 0 1px {p["hair"]}, 0 6px 14px -10px rgba({p["shade"]},{0.30 * k:.2f}); }}
window.hb-window toolbarview > .bottom-bar, window.hb-window actionbar > revealer > box,
window.hb-window .hb-statusbar, window.hb-window .hbf-statusbar {{ {glass} margin: 0 12px 12px;
    border-radius: 16px; box-shadow: {p["spec"]}, {big_shadow}; }}
.hb-surface {{ border-top-left-radius: 24px; border-color: {p["rim"]}; background-color: {p["surface"]}; background-image: none; }}
.hb-card, .hb-card-group, .hb-tile, flowbox.hbf-cards > flowboxchild, .hb-tm-mem-card, .hb-tm-sweep-tile {{
    {glass} border-radius: 16px; box-shadow: {p["spec"]}, {card_shadow}; }}
.hb-tile:hover, flowbox.hbf-cards > flowboxchild:hover {{ background-color: {p["btn_hover"]}; }}
.hb-tm-sweep-hero {{ border-radius: 24px; box-shadow: {p["spec"]}, {big_shadow}; }}
list.hb-nav > row {{ border-radius: 12px; }}
list.hb-nav > row:selected {{ background-image: none; color: {p["accent_text"]};
    box-shadow: {p["spec"]}; }}
list.hb-nav > row:selected .hb-nav-pill {{ background-image: linear-gradient(to bottom, #6366f1, #8b5cf6); }}
entry.hb-search, searchentry.hb-search {{ border-radius: 12px; border: 1px solid {p["line"]};
    background-color: {p["entry"]}; color: {p["text"]}; box-shadow: {p["spec"]}; }}
entry.hb-search:focus-within, searchentry.hb-search:focus-within {{ box-shadow: 0 0 0 2px #6366f1; }}
button.hb-accent {{ background-image: {PLANNER_ACCENT}; color: #ffffff; border-radius: 12px;
    border: 1px solid rgba(255,255,255,0.25); box-shadow: inset 0 1px rgba(255,255,255,0.30), {PLANNER_SHADOW_ACCENT};
    font-weight: 600; }}
button.hb-accent:hover {{ filter: none; opacity: 0.92; }}
button.hb-accent:active, button.hb-plain:active {{ transform: scale(0.95); }}
button.hb-plain {{ background: {p["btn"]}; color: {p["btn_text"]}; border: 1px solid {p["btn_line"]};
    border-radius: 12px; box-shadow: {p["spec"]}; }}
button.hb-plain:hover {{ background: {p["btn_hover"]}; }}
button.hb-tool {{ border-radius: 12px; }}
.hb-badge {{ background-image: {PLANNER_ACCENT}; border-radius: 9999px; }}
.hb-badge.hb-badge-big {{ border-radius: 9999px; }}
.hb-title, .hb-subtitle {{ font-weight: 700; }}
"""


def glass_css(spec, dark, see_through=None):
    """CSS layers for a glass spec, drawn like Fusion Desk's glass: style
    shading, gloss band, corner glare, rim and shadow on every card, tile,
    button, selected nav row and search box."""
    if spec["effect"] == "OFF":
        return ""
    k_full = 1.0 if spec["effect"] == "STRONG" else 0.6
    gloss = spec["material"] == "GLOSS"
    ss, sh, rs, gs, ds, ls = (spec[n] / 100 for n in GLASS_STRENGTHS)
    k = k_full * ss

    def layers(wk_full):
        wk = wk_full * ss
        style = spec["style"]
        out = []
        if gloss:
            out.append(f"linear-gradient(to bottom, {white((0.40 if dark else 0.70) * wk_full * sh)}, "
                       f"{white((0.12 if dark else 0.28) * wk_full * sh)} 18%, {white(0)} 36%)")
        if spec["glare"]:
            g = (0.4 if dark else 0.5) * wk_full * gs * (0.6 if gloss else 1)
            out.append(f"radial-gradient(circle at 6% 0%, {white(g)}, {white(g * 0.35)} 18%, "
                       f"{white(0)} 42%)")
            out.append(f"radial-gradient(circle at 100% 100%, {white(g * 0.7)}, {white(0)} 22%)")
        if style == "FROSTED":
            out.append(f"linear-gradient(to bottom, {white(0.30 * wk)}, {white(0.16 * wk)})")
        elif style == "GEL":
            end = black(0.14 * k) if dark else white(0)
            out.append(f"linear-gradient(to bottom, {white(0.40 * wk)}, {white(0.06 * wk)} 60%, {end})")
        elif style == "TINTED":
            out.append(f"linear-gradient(to bottom, {accent(0.34 * k)}, {accent(0.12 * k)})")
        elif style == "SMOKE":
            out.append(f"linear-gradient(to bottom, {black(0.06 * k)}, {black(0.24 * k)})")
        elif style == "HALO":
            out.append(f"radial-gradient(ellipse at 50% 45%, {white(0.26 * wk)}, {white(0)} 65%)")
        else:  # CLEAR, GLOW
            out.append(f"linear-gradient(to bottom, {white(0.18 * wk)}, {white(0.03 * wk)} 70%, "
                       f"{white(0)})")
        if gloss and dark:
            out.append(f"linear-gradient(to bottom, {black(0)} 60%, {black(0.2 * k_full * sh)})")
        return ", ".join(out)

    def shadows(extra=""):
        parts = []
        if spec["rim"] and rs > 0:
            rk = k_full * rs
            top = (0.6 if dark else 0.95) * rk if gloss else (0.32 if dark else 0.95) * rk
            parts.append(f"inset 0 1px {white(top)}")
            if gloss:
                parts.append(f"inset 0 -1px {black((0.38 if dark else 0.16) * rk)}")
        if spec["shadow"] and ds > 0:
            parts.append(f"0 {round(4 * ds)}px {round(14 * ds)}px {black((0.45 if dark else 0.18) * ds)}")
        if extra:
            parts.append(extra)
        return ", ".join(parts) or "none"

    wk = (0.4 if dark else 1.0) * k_full
    # Shared Harbor classes plus the cards, panes and bars Files and Task
    # Manager paint with their own classes, so every glass option shows there.
    neutral = (".hb-card, .hb-tile, button.hb-plain, .hb-surface, list.hb-nav > row:selected, "
               "entry.hb-search, searchentry.hb-search, "
               "flowbox.hbf-cards > flowboxchild, .hbf-pane, .hbf-cmdbar, .hbf-ribbon, "
               ".hbf-address, .hbf-titlebar tabbar tab:selected, "
               ".hb-tm-pane, .hb-tm-mem-card, .hb-tm-sweep-hero, .hb-tm-sweep-tile")
    css = (f"{neutral} {{ background-image: {layers(wk)}; box-shadow: {shadows()}; }}\n"
           f"button.hb-accent, .hb-badge {{ background-image: {layers(wk * 0.7)}; "
           f"box-shadow: {shadows()}; }}\n")
    if spec["light-follows"]:
        spot = f"radial-gradient(circle at 50% 50%, {white((0.12 if dark else 0.35) * ls)}, {white(0)} 70%)"
        css += ("button.hb-card:hover, button.hb-tile:hover, button.hb-plain:hover, "
                f"list.hb-nav > row:hover {{ background-image: {spot}, {layers(wk)}; }}\n")
    if see_through is None:
        see_through = spec["opacity"] < 100
    if spec["style"] == "GLOW":
        a = (0.42 if dark else 0.26) * ss
        blobs = ", ".join(
            f"radial-gradient(circle at {x}% {y}%, {c(a)}, {c(0)} {r}%)"
            for c, x, y, r in ((lambda v: "rgba(150,200,40,%.2f)" % v, 12, 2, 50),
                               (lambda v: "rgba(20,170,170,%.2f)" % v, 42, 80, 45),
                               (lambda v: "rgba(230,110,90,%.2f)" % v, 100, 95, 50),
                               (accent, 85, 10, 38)))
        css += f"window.hb-window {{ background-image: {blobs}; }}\n"
    if see_through or spec["style"] == "GLOW":
        css += (".hb-window toolbarview, .hb-window scrolledwindow, .hb-window viewport, "
                ".hb-window columnview, .hb-window listview, .hb-window gridview, "
                ".hb-window .view, .hb-window textview text, .hb-window headerbar, "
                ".hb-window > contents, .hb-content, .hb-sidebar, .hb-window .sidebar-pane, "
                ".hb-window .content-pane, .hb-window navigation-view, "
                ".hb-window overlay-split-view, .hb-window list.navigation-sidebar, "
                ".hb-window .hb-tm-navcol "
                "{ background-color: transparent; }\n")
    if spec["style"] == "PLANNER":
        css += _planner_css(spec, dark, see_through)
    return css


class _Theme:
    def __init__(self):
        self.provider = Gtk.CssProvider()
        self.extra = []
        self.style = "harbor"      # "harbor" (own colours) or "system"
        self.app = None            # "files" / "taskmgr": reads ~/.config/harbor/glass-<app>
        self.planner = False       # the Lumina Glass (PLANNER) look is on
        self.on_planner = []       # callbacks when that changes (Files: glossy icons)
        self._monitors = []
        self._fusion = None
        self.manager = Adw.StyleManager.get_default()
        self.manager.connect("notify::dark", lambda *_: self.reload())

    @property
    def dark(self):
        return self.manager.get_dark()

    def palette(self):
        if self.style == "system":
            return PALETTES["system"]
        return PALETTES["dark" if self.dark else "light"]

    @property
    def glass(self):
        """The glass dict this app paints with. HARBOR_GLASS (off, glossy,
        liquid or key=value,...) overrides it for screenshot runs."""
        if os.environ.get("HARBOR_GLASS"):
            return parse_glass(os.environ["HARBOR_GLASS"])
        if self.app is None:
            return dict(GLASS_OFF)
        return resolve_glass(self.app, self._fusion)

    def set_app(self, app):
        """Name this app ("files", "taskmgr") so it reads its glass file."""
        if app != self.app:
            self.app = app
            self.watch_glass()
            self.reload()

    def set_glass(self, choice):
        """Save this app's quick glass choice (see GLASS_APP_CHOICES)."""
        if self.app and choice in GLASS_APP_CHOICES:
            set_app_glass_choice(self.app, choice)
            self.reload()

    def glass_choice(self):
        return app_glass_choice(self.app) if self.app else "harbor"

    def watch_glass(self):
        """Reload when Harbor Settings or Fusion Desk change the glass."""
        if self._monitors or self.app is None:
            return
        for name in ("glass", "glass-global", "glass-" + self.app):
            path = harbor_config_path(name)
            try:
                os.makedirs(os.path.dirname(path), exist_ok=True)
                mon = Gio.File.new_for_path(path).monitor_file(Gio.FileMonitorFlags.NONE, None)
                mon.connect("changed", lambda *_: self.reload())
                self._monitors.append(mon)
            except GLib.Error:
                pass
        self._fusion = fusion_settings()
        if self._fusion is not None:
            self._fusion.connect(
                "changed", lambda _s, key: (key.startswith("glass-") or key == "harbor-glass-blur")
                and self.reload())
            try:
                self._shell = Gio.Settings(schema_id="org.gnome.shell")
                self._shell.connect("changed::enabled-extensions", lambda *_: self.reload())
            except Exception:
                self._shell = None

    def set_style(self, style):
        style = "system" if style == "system" else "harbor"
        if style != self.style:
            self.style = style
            self.reload()

    def reload(self):
        spec = self.glass
        see = spec["opacity"] < 100 and fusion_blurs_windows(self._fusion)
        pal = glass_palette(self.palette(), spec, self.dark, see)
        css = CSS.substitute(pal)
        for tmpl in self.extra:
            css += tmpl.substitute(pal)
        css += glass_css(spec, self.dark, see)
        self.provider.load_from_string(css)
        planner = spec.get("effect") != "OFF" and spec.get("style") == "PLANNER"
        if planner != self.planner:
            self.planner = planner
            for cb in list(self.on_planner):
                cb()


THEME = None


def apply_theme(extra_css=""):
    """Install the Harbor CSS once. extra_css may use $window, $card, ... ."""
    global THEME
    if THEME is None:
        THEME = _Theme()
        display = Gdk.Display.get_default()
        Gtk.StyleContext.add_provider_for_display(
            display, THEME.provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
    if extra_css:
        THEME.extra.append(string.Template(extra_css))
    THEME.reload()
    return THEME


def heat_class(fraction):
    """CSS class for a Task Manager style heat cell (0.0 .. 1.0)."""
    if fraction >= 0.6:
        return "hb-heat3"
    if fraction >= 0.25:
        return "hb-heat2"
    if fraction >= 0.03:
        return "hb-heat1"
    return None


# ------------------------------------------------------------------ window shell

class HarborWindow(Adw.ApplicationWindow):
    """Window with a Windows-style title bar (title left, controls right)."""

    def __init__(self, app, title, width=1100, height=720, icon=None):
        super().__init__(application=app, title=title)
        self.add_css_class("hb-window")
        self.set_default_size(width, height)
        self.toolbar = Adw.ToolbarView()
        self.header = Adw.HeaderBar()
        self.header.set_decoration_layout(":minimize,maximize,close")
        tbox = Gtk.Box(spacing=8)
        if icon:
            img = Gtk.Image.new_from_icon_name(icon)
            img.set_pixel_size(16)
            tbox.append(img)
        self.title_label = Gtk.Label(label=title, css_classes=["hb-small"])
        tbox.append(self.title_label)
        self.header.pack_start(tbox)
        self.header.set_title_widget(Gtk.Box())
        self.toolbar.add_top_bar(self.header)
        self.toast = Adw.ToastOverlay()
        self.toolbar.set_content(self.toast)
        self.set_content(self.toolbar)

    def set_body(self, widget):
        self.toast.set_child(widget)

    def notify_toast(self, text, timeout=3):
        self.toast.add_toast(Adw.Toast(title=text, timeout=timeout))


# ------------------------------------------------------------------ navigation list

class NavList(Gtk.ListBox):
    """Win11 navigation: icon + label, accent pill on the selected row.

    items: list of (key, label, icon_name). Use add_heading() for group titles.
    Emits "selected" (key) when the user picks a row.
    """

    __gsignals__ = {"selected": (GObject.SignalFlags.RUN_FIRST, None, (str,))}

    def __init__(self, items=(), badges=False):
        super().__init__(css_classes=["hb-nav"], selection_mode=Gtk.SelectionMode.SINGLE)
        self.badges = badges
        self.rows = {}
        self._quiet = False
        for key, label, icon in items:
            self.add_item(key, label, icon)
        self.connect("row-selected", self._on_row)

    def add_item(self, key, label, icon, suffix=None):
        box = Gtk.Box(spacing=12, margin_start=4, margin_end=10)
        pill = Gtk.Box(css_classes=["hb-nav-pill"], valign=Gtk.Align.CENTER)
        box.append(pill)
        if self.badges:
            box.append(icon_badge(icon))
        else:
            img = Gtk.Image.new_from_icon_name(icon)
            img.set_pixel_size(16)
            box.append(img)
        lab = Gtk.Label(label=label, xalign=0, hexpand=True, css_classes=["hb-nav-label"],
                        ellipsize=3)
        box.append(lab)
        if suffix is not None:
            box.append(suffix)
        row = Gtk.ListBoxRow(child=box)
        row.key = key
        row.label = lab
        self.append(row)
        self.rows[key] = row
        return row

    def add_heading(self, text):
        row = Gtk.ListBoxRow(selectable=False, activatable=False)
        row.set_child(Gtk.Label(label=text, xalign=0, css_classes=["hb-nav-heading"]))
        row.key = None
        self.append(row)
        return row

    def select(self, key, emit=False):
        row = self.rows.get(key)
        if row is None:
            self.unselect_all()
            return
        self._quiet = not emit
        self.select_row(row)
        self._quiet = False

    def _on_row(self, _lb, row):
        if row is not None and row.key and not self._quiet:
            self.emit("selected", row.key)


# ------------------------------------------------------------------ small widgets

def icon_badge(icon_name, big=False, color=None):
    """Accent-coloured rounded square with a white symbolic icon."""
    # homogeneous centres the icon without hexpand (hexpand would spread
    # to every parent box and stretch the row)
    box = Gtk.Box(css_classes=["hb-badge"] + (["hb-badge-big"] if big else []),
                  halign=Gtk.Align.CENTER, valign=Gtk.Align.CENTER, homogeneous=True)
    img = Gtk.Image.new_from_icon_name(icon_name)
    img.set_pixel_size(20 if big else 14)
    box.append(img)
    if color:
        prov = Gtk.CssProvider()
        prov.load_from_string(".hb-badge { background: %s; }" % color)
        box.get_style_context().add_provider(prov, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION + 1)
    return box


def label(text, *classes, xalign=0, wrap=False, **kw):
    lab = Gtk.Label(label=text, xalign=xalign, css_classes=list(classes), **kw)
    if wrap:
        lab.set_wrap(True)
        lab.set_wrap_mode(2)  # WORD_CHAR
    return lab


def title(text):
    return label(text, "hb-title")


def section(text):
    return label(text, "hb-section")


class Card(Gtk.Box):
    """A Windows settings row: [icon] title / subtitle ........ [suffix] [>].

    on_activate makes the whole card a button with a chevron (unless
    chevron=False). suffix is any widget (Gtk.Switch, Gtk.DropDown, button).
    """

    def __init__(self, title, subtitle=None, icon=None, suffix=None, on_activate=None,
                 chevron=None, badge=False):
        super().__init__()
        inner = Gtk.Box(spacing=14)
        if icon:
            if badge:
                inner.append(icon_badge(icon))
            else:
                img = Gtk.Image.new_from_icon_name(icon)
                img.set_pixel_size(18)
                inner.append(img)
        texts = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, hexpand=True,
                        valign=Gtk.Align.CENTER)
        self.title_label = label(title, "hb-card-title", ellipsize=3)
        texts.append(self.title_label)
        self.sub_label = label(subtitle or "", "hb-card-sub", ellipsize=3)
        self.sub_label.set_visible(bool(subtitle))
        texts.append(self.sub_label)
        inner.append(texts)
        self.suffix_box = Gtk.Box(spacing=10, valign=Gtk.Align.CENTER)
        if suffix is not None:
            self.suffix_box.append(suffix)
        inner.append(self.suffix_box)
        if chevron is None:
            chevron = on_activate is not None and suffix is None
        if chevron:
            chev = Gtk.Image.new_from_icon_name("go-next-symbolic")
            chev.add_css_class("hb-dim")
            inner.append(chev)
        if on_activate is not None:
            btn = Gtk.Button(child=inner, css_classes=["hb-card"], hexpand=True)
            btn.connect("clicked", lambda *_: on_activate())
            self.append(btn)
        else:
            inner.add_css_class("hb-card")
            inner.set_hexpand(True)
            self.append(inner)

    def set_subtitle(self, text):
        self.sub_label.set_label(text or "")
        self.sub_label.set_visible(bool(text))

    def set_title(self, text):
        self.title_label.set_label(text)


def card_group(*cards, spacing=3):
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=spacing,
                  css_classes=["hb-card-group"])
    for c in cards:
        box.append(c)
    return box


def switch(active=False, on_toggle=None):
    """Gtk.Switch with an On/Off label in front, like Windows."""
    box = Gtk.Box(spacing=10, valign=Gtk.Align.CENTER)
    lab = label("On" if active else "Off", "hb-dim")
    sw = Gtk.Switch(active=active, valign=Gtk.Align.CENTER)

    def changed(s, _p):
        lab.set_label("On" if s.get_active() else "Off")
        if on_toggle and not getattr(sw, "_quiet", False):
            on_toggle(s.get_active())
    sw.connect("notify::active", changed)
    box.append(lab)
    box.append(sw)
    box.switch = sw

    def set_quiet(value):
        sw._quiet = True
        sw.set_active(value)
        sw._quiet = False
    box.set_quiet = set_quiet
    return box


def dropdown(options, selected=0, on_change=None):
    dd = Gtk.DropDown.new_from_strings(list(options))
    dd.set_selected(selected)
    dd.set_valign(Gtk.Align.CENTER)
    if on_change:
        dd.connect("notify::selected", lambda d, _p: on_change(d.get_selected()))
    return dd


def scrolled(child, hscroll=False):
    sw = Gtk.ScrolledWindow(vexpand=True, hexpand=True, child=child)
    sw.set_policy(Gtk.PolicyType.AUTOMATIC if hscroll else Gtk.PolicyType.NEVER,
                  Gtk.PolicyType.AUTOMATIC)
    return sw


def page_box(max_width=1000, margin=36):
    """Centered, width-limited vertical page body inside a scroller."""
    clamp = Adw.Clamp(maximum_size=max_width, tightening_threshold=max_width)
    body = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4,
                   margin_start=margin, margin_end=margin, margin_top=18, margin_bottom=28)
    clamp.set_child(body)
    return scrolled(clamp), body


# ------------------------------------------------------------------ helpers

def human_size(n, decimals=1):
    n = float(n or 0)
    for unit in ("B", "KB", "MB", "GB", "TB"):
        if abs(n) < 1024 or unit == "TB":
            if unit == "B":
                return "%d B" % n
            return ("%." + str(decimals) + "f %s") % (n, unit)
        n /= 1024.0
    return "%d B" % n


def human_rate(n):
    return human_size(n) + "/s"


def run_cmd(argv, callback=None, stdin=None, mutating=True):
    """Run argv asynchronously; callback(ok, stdout, stderr).

    Commands that change the system must keep mutating=True so that
    HARBOR_DRY_RUN=1 skips them. Read-only queries pass mutating=False.
    """
    if DRY_RUN and mutating:
        DRY_RUN_LOG.append(list(argv))
        print("[dry-run]", shlex.join(argv), file=sys.stderr)
        if callback:
            GLib.idle_add(lambda: (callback(True, "", ""), False)[1])
        return None
    flags = Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE
    if stdin is not None:
        flags |= Gio.SubprocessFlags.STDIN_PIPE
    try:
        proc = Gio.Subprocess.new(argv, flags)
    except GLib.Error as e:
        if callback:
            GLib.idle_add(lambda: (callback(False, "", e.message), False)[1])
        return None

    def done(p, res):
        try:
            _ok, out, err = p.communicate_utf8_finish(res)
            ok = p.get_successful()
        except GLib.Error as e:
            ok, out, err = False, "", e.message
        if callback:
            callback(ok, out or "", err or "")
    proc.communicate_utf8_async(stdin, None, done)
    return proc


def run_sync(argv, timeout=5):
    """Read-only query, returns stdout or "" on any failure."""
    import subprocess
    try:
        return subprocess.run(argv, capture_output=True, text=True, timeout=timeout).stdout
    except (OSError, subprocess.SubprocessError):
        return ""


def spawn(argv):
    """Start a program and forget it (honours HARBOR_DRY_RUN)."""
    if DRY_RUN:
        DRY_RUN_LOG.append(list(argv))
        print("[dry-run] spawn", shlex.join(argv), file=sys.stderr)
        return True
    try:
        GLib.spawn_async(argv, flags=GLib.SpawnFlags.SEARCH_PATH)
        return True
    except GLib.Error:
        return False


def have(program):
    return GLib.find_program_in_path(program) is not None


def greeting(hour=None):
    if hour is None:
        hour = GLib.DateTime.new_now_local().get_hour()
    if hour < 5:
        return "Good evening"
    if hour < 12:
        return "Good morning"
    if hour < 18:
        return "Good afternoon"
    return "Good evening"


def user_display_name():
    name = GLib.get_real_name()
    if not name or name == "Unknown":
        name = GLib.get_user_name()
    return name


def about_dialog(parent, app_name, icon, comment):
    dlg = Adw.AboutDialog(application_name=app_name, application_icon=icon,
                          version=VERSION, developer_name="Md. Hafijur Rahman",
                          comments=comment, license_type=Gtk.License.MIT_X11)
    dlg.present(parent)
