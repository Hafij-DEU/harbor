#!/usr/bin/env python3
"""Write Harbor's own full-colour ribbon icons (harbor-rb-*.svg).

Flat, Fluent-like 32x32 drawings made from simple shapes so they stay sharp
at 16 px and 32 px. Run from the repo root; output goes to
data/icons/hicolor/scalable/actions/.
"""

import os

OUT = os.path.join("data", "icons", "hicolor", "scalable", "actions")

BLUE, BLUE_D = "#3b8eea", "#1f6fd1"
GREEN, GREEN_D = "#2fb35c", "#1e8e44"
RED, RED_D = "#ef4444", "#c62828"
AMBER, AMBER_D = "#ffcf5a", "#e9a823"
ORANGE = "#f08a24"
GREY, GREY_D = "#c9d1dc", "#7b8796"
PAGE, PAGE_E = "#ffffff", "#9aa6b5"
PURPLE = "#8b5cf6"


def g(name, a, b):
    return ('<linearGradient id="%s" x1="0" y1="0" x2="0" y2="1"><stop offset="0" '
            'stop-color="%s"/><stop offset="1" stop-color="%s"/></linearGradient>' % (name, a, b))


def svg(body, defs=""):
    return ('<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32">'
            '<defs>%s</defs>%s</svg>' % (defs, body))


def page(x, y, w, h, lines=0, fold=True, fill=PAGE, line=BLUE):
    f = 5 if fold else 0
    d = "M%g %gh%gl%g %gv%gh-%gz" % (x, y, w - f, f, f, h - f, w)
    out = '<path d="%s" fill="%s" stroke="%s" stroke-width="1.2" stroke-linejoin="round"/>' % (
        d, fill, PAGE_E)
    if fold:
        out += '<path d="M%g %gv%gh%g" fill="none" stroke="%s" stroke-width="1.2"/>' % (
            x + w - f, y, f, f, PAGE_E)
    for i in range(lines):
        out += '<rect x="%g" y="%g" width="%g" height="1.6" rx=".8" fill="%s"/>' % (
            x + 3, y + 7 + i * 3.4, w - 6 - (3 if i == lines - 1 else 0), line)
    return out


def folder(x=2, y=7, w=28, h=20, defs_id="fd"):
    """Modern (Fluent-like) folder: rounded back with a soft tab, glossy front."""
    back = ('<path d="M%g %ga2.5 2.5 0 0 1 2.5-2.5h6.2a2.5 2.5 0 0 1 1.9.9l1.6 1.9H%ga2.5 2.5 0 0 1 '
            '2.5 2.5V%gH%gz" fill="url(#%sb)"/>' % (x, y + 2.5, x + w - 2.5, y + h - 3, x, defs_id))
    front = ('<rect x="%g" y="%g" width="%g" height="%g" rx="2.6" fill="url(#%sf)"/>'
             '<rect x="%g" y="%g" width="%g" height="1.1" rx=".55" fill="#fff" opacity=".55"/>' % (
                 x, y + 5.5, w, h - 5.5, defs_id, x + 1.5, y + 6.2, w - 3))
    return back + front, g(defs_id + "b", "#f2b13a", "#d98f12") + g(defs_id + "f", "#ffe08a", "#f7b928")


def card(x, y, w, h, lines=2, line=BLUE):
    out = ('<rect x="%g" y="%g" width="%g" height="%g" rx="2.2" fill="#fff" stroke="%s" '
           'stroke-width="1"/>' % (x, y, w, h, PAGE_E))
    for i in range(lines):
        out += '<rect x="%g" y="%g" width="%g" height="1.5" rx=".75" fill="%s"/>' % (
            x + 2.2, y + 3 + i * 3, w - 4.4 - (2.5 if i == lines - 1 else 0), line)
    return out


def badge(cx, cy, color, glyph):
    """A round badge with a white glyph: plus, arrow, check, x."""
    shapes = {
        "plus": '<path d="M%g %gh6M%g %gv6" stroke="#fff" stroke-width="2" stroke-linecap="round"/>' % (
            cx - 3, cy, cx, cy - 3),
        "arrow": '<path d="M%g %gh6M%g %gl3 3-3 3" fill="none" stroke="#fff" stroke-width="2" '
                 'stroke-linecap="round" stroke-linejoin="round"/>' % (cx - 3.5, cy, cx, cy - 3),
        "check": '<path d="M%g %gl2.2 2.2 4-4.4" fill="none" stroke="#fff" stroke-width="2" '
                 'stroke-linecap="round" stroke-linejoin="round"/>' % (cx - 3.2, cy),
    }
    return '<circle cx="%g" cy="%g" r="6.5" fill="%s" stroke="#fff" stroke-width="1.2"/>%s' % (
        cx, cy, color, shapes[glyph])


def window(inner="", title=BLUE):
    return ('<rect x="3" y="5" width="26" height="22" rx="2.5" fill="#fff" stroke="%s" '
            'stroke-width="1.2"/><path d="M3 7.5a2.5 2.5 0 0 1 2.5-2.5h21a2.5 2.5 0 0 1 2.5 2.5V10H3z" '
            'fill="%s"/>%s' % (PAGE_E, title, inner))


def make():
    icons = {}
    fb, fdefs = folder()

    icons["pin"] = svg(
        '<path d="M19.5 3.5l9 9-3 1.2-4.6 4.6.6 5.2-2.3 2.3-5.2-5.2L7 27.5 4.5 25l6.9-7-5.2-5.2 '
        '2.3-2.3 5.2.6 4.6-4.6z" fill="url(#p)" stroke="%s" stroke-width="1" '
        'stroke-linejoin="round"/>' % BLUE_D, g("p", "#62a8f5", BLUE_D))
    icons["copy"] = svg(
        '<rect x="3.5" y="2.5" width="17" height="21" rx="3.2" fill="url(#cb)"/>'
        '<rect x="10.5" y="8.5" width="18" height="21" rx="3.2" fill="#000" opacity=".12"/>'
        '<rect x="10" y="7.5" width="18" height="21" rx="3.2" fill="#fff" stroke="%s" '
        'stroke-width="1"/>' % PAGE_E + "".join(
            '<rect x="13.5" y="%g" width="%g" height="1.8" rx=".9" fill="%s"/>' % (
                12 + i * 3.6, 11 - (4 if i == 3 else 0), BLUE) for i in range(4)),
        g("cb", "#8cc4fb", BLUE))
    icons["paste"] = svg(
        '<rect x="4" y="5" width="20" height="24" rx="2.5" fill="url(#c)"/>'
        '<rect x="9" y="2.5" width="10" height="5" rx="1.5" fill="%s"/>' % GREY_D
        + page(12, 11, 16, 18, 3), g("c", "#d9a066", "#b5763a"))
    icons["cut"] = svg(
        '<path d="M11.2 2.5c.9 0 1.6.5 2 1.3l7 15.4-2.6 1.3-7.4-14.6a2.4 2.4 0 0 1 1-3.4z" fill="url(#bl)"/>'
        '<path d="M20.8 2.5c-.9 0-1.6.5-2 1.3l-7 15.4 2.6 1.3 7.4-14.6a2.4 2.4 0 0 0-1-3.4z" fill="url(#bl)"/>'
        '<circle cx="16" cy="16.2" r="1.3" fill="%s"/>'
        '<circle cx="10" cy="24" r="4.3" fill="none" stroke="url(#hd)" stroke-width="3"/>'
        '<circle cx="22" cy="24" r="4.3" fill="none" stroke="url(#hd)" stroke-width="3"/>' % GREY_D,
        g("bl", "#eef2f7", "#9aa6b5") + g("hd", "#5aa7f5", BLUE_D))
    icons["link"] = svg(
        '<g fill="none" stroke-width="3" stroke-linecap="round"><path d="M13.5 18.5l5-5" '
        'stroke="%s"/><path d="M11 15l-3.5 3.5a4.6 4.6 0 0 0 6.5 6.5L17.5 21.5" stroke="%s"/>'
        '<path d="M21 17l3.5-3.5a4.6 4.6 0 0 0-6.5-6.5L14.5 10.5" stroke="%s"/></g>' % (
            GREY_D, BLUE, BLUE_D))
    icons["shortcut"] = svg(page(6, 3, 20, 26, 2) + '<rect x="9" y="16" width="11" height="11" '
                            'rx="2" fill="%s"/><path d="M12 24c0-4 2-5.5 5-5.5M15 16.5l2.2 2-2.2 2" '
                            'fill="none" stroke="#fff" stroke-width="1.7" stroke-linecap="round" '
                            'stroke-linejoin="round"/>' % BLUE)
    def chip(color, glyph):
        """Rounded-square chip with a white ring, bottom right."""
        return ('<rect x="16.5" y="15.5" width="14" height="14" rx="4" fill="%s" stroke="#fff" '
                'stroke-width="1.6"/>' % color + glyph)
    icons["move-to"] = svg(fb + chip(BLUE, '<path d="M19.8 22.5h7.2M24 19.4l3.1 3.1-3.1 3.1" '
                                     'fill="none" stroke="#fff" stroke-width="2" stroke-linecap="round" '
                                     'stroke-linejoin="round"/>'), fdefs)
    icons["copy-to"] = svg(fb + chip(GREEN, '<rect x="22" y="18.6" width="5.6" height="6.4" rx="1.3" '
                                     'fill="none" stroke="#fff" stroke-width="1.6"/><path d="M20 21.3v4'
                                     'a1.4 1.4 0 0 0 1.4 1.4h3.6" fill="none" stroke="#fff" '
                                     'stroke-width="1.6" stroke-linecap="round"/>'), fdefs)
    icons["delete"] = svg(
        '<path d="M6.5 6.5l19 19M25.5 6.5l-19 19" stroke="url(#r)" stroke-width="4.6" '
        'stroke-linecap="round"/>', g("r", "#ff6b6b", RED_D))
    icons["rename"] = svg(
        '<rect x="2.5" y="10" width="27" height="12" rx="2" fill="#fff" stroke="%s" stroke-width="1.2"/>'
        '<rect x="5.5" y="14" width="12" height="4" rx="1" fill="%s"/>'
        '<path d="M22 7.5v17M20 7.5h4M20 24.5h4" stroke="#1d2733" stroke-width="1.5" '
        'stroke-linecap="round"/>' % (PAGE_E, BLUE))
    icons["new-folder"] = svg(fb + chip(GREEN, '<path d="M23.5 18.9v7.2M19.9 22.5h7.2" stroke="#fff" '
                                        'stroke-width="2" stroke-linecap="round"/>'), fdefs)
    icons["new-item"] = svg(page(5, 3, 19, 25, 3) + badge(23, 23, GREEN, "plus"))
    icons["properties"] = svg(page(5, 3, 20, 26, 3) + '<path d="M11 20l3.5 3.5L27 9" fill="none" '
                              'stroke="%s" stroke-width="3.2" stroke-linecap="round" '
                              'stroke-linejoin="round"/>' % RED)
    icons["open"] = svg(
        '<path d="M2 8h8l2.5 3H26v4H2z" fill="#dc9420"/>'
        '<path d="M2 26V9h0l0 0M6 15h24l-4 12H2z" fill="url(#o)"/>' + page(8, 6, 14, 10, 1, False),
        g("o", "#ffdc7a", "#f7bd3c"))
    icons["open-with"] = svg(''.join(
        '<rect x="%g" y="%g" width="11" height="11" rx="3" fill="%s"/>' % (x, y, c)
        for x, y, c in ((4, 4, BLUE), (17, 4, GREEN), (4, 17, ORANGE), (17, 17, PURPLE))))

    def grid(fill_fn):
        out = ""
        for r in range(3):
            for c in range(3):
                f = fill_fn(r, c)
                out += '<rect x="%g" y="%g" width="7" height="7" rx="1.5" fill="%s" stroke="%s" ' \
                       'stroke-width="1"/>' % (4 + c * 8.5, 4 + r * 8.5, f, BLUE_D if f != "#fff" else PAGE_E)
        return out
    icons["select-all"] = svg(grid(lambda r, c: BLUE))
    icons["select-none"] = svg(grid(lambda r, c: "#fff"))
    icons["invert"] = svg(grid(lambda r, c: BLUE if (r + c) % 2 else "#fff"))

    icons["window-new"] = svg(window(badge(22, 21, GREEN, "plus")))
    icons["terminal"] = svg(
        '<rect x="3" y="5" width="26" height="22" rx="3" fill="#1f2937"/>'
        '<path d="M8 12l4.5 4L8 20" fill="none" stroke="%s" stroke-width="2.2" stroke-linecap="round" '
        'stroke-linejoin="round"/><path d="M15 21h8" stroke="#e5e7eb" stroke-width="2.2" '
        'stroke-linecap="round"/>' % GREEN)
    teeth = "".join('<rect x="14" y="2.5" width="4" height="6" rx="1" fill="url(#gr)" '
                    'transform="rotate(%d 16 16)"/>' % a for a in range(0, 360, 45))
    icons["options"] = svg(teeth + '<circle cx="16" cy="16" r="10" fill="url(#gr)"/>'
                           '<circle cx="16" cy="16" r="4" fill="#fff"/>', g("gr", "#9aa6b5", "#5b6675"))
    icons["close"] = svg(window('<path d="M12 14l8 8M20 14l-8 8" stroke="%s" stroke-width="2.6" '
                                'stroke-linecap="round"/>' % RED, title=GREY_D))
    icons["zip"] = svg(
        '<rect x="5" y="3" width="22" height="26" rx="2.5" fill="url(#z)"/>'
        + "".join('<rect x="%g" y="%g" width="3" height="2" fill="#7a4a12"/>' % (
            14 + (i % 2) * 2, 4 + i * 2.2) for i in range(6))
        + '<rect x="13" y="17" width="6" height="7" rx="1.5" fill="#fff" stroke="#7a4a12"/>',
        g("z", "#f6c66b", "#d9932b"))
    icons["wifi"] = svg(
        '<g fill="none" stroke-linecap="round" stroke-width="3"><path d="M4 12.5a17 17 0 0 1 24 0" '
        'stroke="%s"/><path d="M8.5 17a10.5 10.5 0 0 1 15 0" stroke="%s"/><path d="M12.8 21.4a4.6 '
        '4.6 0 0 1 6.4 0" stroke="%s"/></g><circle cx="16" cy="25.5" r="2.2" fill="%s"/>' % (
            "#8cc4fb", BLUE, BLUE_D, BLUE_D))
    icons["usb"] = svg(
        '<g fill="none" stroke-linecap="round" stroke-linejoin="round" stroke-width="2.6">'
        '<path d="M16 7v19" stroke="%s"/><path d="M16 21l-6.5-4.5V13" stroke="%s"/>'
        '<path d="M16 18.5l6.5-4V11" stroke="%s"/></g>'
        '<path d="M16 2.5l3.6 5.2h-7.2z" fill="%s"/><circle cx="9.5" cy="11.5" r="2.4" fill="%s"/>'
        '<rect x="20.3" y="7.6" width="4.4" height="4.4" rx="1" fill="%s"/>'
        '<circle cx="16" cy="27" r="3" fill="%s"/>' % (
            BLUE, "#8cc4fb", BLUE, BLUE_D, "#8cc4fb", BLUE, BLUE_D))
    icons["send"] = svg(
        '<path d="M3 15L29 4 22 28l-6-9z" fill="url(#s)"/><path d="M16 19l13-15-17 13z" '
        'fill="%s"/>' % BLUE_D, g("s", "#62a8f5", BLUE))

    def qr_corner(x, y):
        return ('<rect x="%g" y="%g" width="9" height="9" rx="1.5" fill="none" stroke="#1f2937" '
                'stroke-width="2"/><rect x="%g" y="%g" width="3.6" height="3.6" fill="%s"/>' % (
                    x, y, x + 2.7, y + 2.7, BLUE))
    icons["qr"] = svg('<rect x="2" y="2" width="28" height="28" rx="4" fill="#fff" stroke="%s"/>'
                      % PAGE_E + qr_corner(5, 5) + qr_corner(18, 5) + qr_corner(5, 18)
                      + "".join('<rect x="%g" y="%g" width="3" height="3" fill="#1f2937"/>' % p
                                for p in ((19, 19), (24, 19), (21.5, 22.5), (19, 25), (24, 25))))
    icons["nav-pane"] = svg(window('<rect x="4" y="10.5" width="8" height="15.8" fill="%s" '
                                   'opacity=".85"/>' % BLUE))
    icons["split"] = svg(window('<rect x="4.6" y="11.5" width="10.6" height="14" rx="1" fill="%s" '
                                'opacity=".85"/><rect x="16.8" y="11.5" width="10.6" height="14" rx="1" '
                                'fill="%s" opacity=".85"/>' % (BLUE, ORANGE)))
    icons["preview-pane"] = svg(window('<rect x="18" y="10.5" width="10.4" height="15.8" fill="%s" '
                                       'opacity=".85"/><path d="M20 21l2.5-3 2 2 1.5-1.5 1.5 2.5z" '
                                       'fill="#fff"/>' % ORANGE))
    icons["details-pane"] = svg(window('<rect x="18" y="10.5" width="10.4" height="15.8" fill="%s" '
                                       'opacity=".2"/>' % PURPLE + "".join(
                                           '<rect x="19.5" y="%g" width="7" height="1.8" rx=".9" '
                                           'fill="%s"/>' % (13 + i * 3.5, PURPLE) for i in range(3))))
    icons["large-icons"] = svg(''.join(
        '<rect x="%g" y="%g" width="11.5" height="11.5" rx="2.5" fill="%s"/>' % (x, y, c)
        for x, y, c in ((3.5, 3.5, BLUE), (17, 3.5, AMBER_D), (3.5, 17, GREEN), (17, 17, PURPLE))))
    icons["medium-icons"] = svg(''.join(
        '<rect x="%g" y="%g" width="7" height="7" rx="1.8" fill="%s"/>' % (
            3.5 + c * 9, 3.5 + r * 9, (BLUE, AMBER_D, GREEN)[(r + c) % 3])
        for r in range(3) for c in range(3)))
    icons["details-view"] = svg(''.join(
        '<rect x="3.5" y="%g" width="4.5" height="4.5" rx="1" fill="%s"/>'
        '<rect x="10" y="%g" width="18.5" height="2.4" rx="1.2" fill="%s"/>' % (
            4 + i * 6.5, (BLUE, AMBER_D, GREEN, PURPLE)[i], 5 + i * 6.5, GREY_D)
        for i in range(4)))
    icons["sort"] = svg(
        '<g fill="none" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round">'
        '<path d="M10 27V5M5 10l5-5 5 5" stroke="%s"/><path d="M22 5v22M17 22l5 5 5-5" '
        'stroke="%s"/></g>' % (BLUE, ORANGE))
    icons["edit"] = svg(page(4, 3, 19, 25, 4) + '<path d="M27.5 10.5l2.5 2.5-11 11-3.6 1 1-3.6z" '
                        'fill="%s" stroke="#9c4f0a" stroke-width=".8" stroke-linejoin="round"/>' % ORANGE)
    return icons


def main():
    os.makedirs(OUT, exist_ok=True)
    icons = make()
    for name, text in icons.items():
        with open(os.path.join(OUT, "harbor-rb-%s.svg" % name), "w") as f:
            f.write(text + "\n")
    print("wrote %d icons to %s" % (len(icons), OUT))


if __name__ == "__main__":
    main()
