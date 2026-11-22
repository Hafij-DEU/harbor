#!/usr/bin/env python3
"""Write the Custom design's ribbon icons (harbor-rbc-*.svg).

Flat circles in bright colours with a white glyph and a 45 degree long
shadow, one icon for every harbor-rb-* name. The glyph silhouette lives in
<defs> and is stamped many times along the diagonal (clipped to the circle)
to make the shadow; colour details are drawn on top. Run from the repo root.

    python3 tools/make_custom_icons.py
"""

import math
import os

OUT = os.path.join("data", "icons", "hicolor", "scalable", "actions")

RED, ORANGE, AMBER = "#e8445a", "#f2673a", "#f5b300"
GREEN, TEAL, LBLUE = "#16b88a", "#10a3a0", "#1ba1e2"
BLUE, PINK, PURPLE = "#1673d6", "#d94fbf", "#8e63d6"
GREY, DARK = "#9aa3ad", "#4b5563"

F = 'fill="currentColor"'
S = ('fill="none" stroke="currentColor" stroke-width="%g" stroke-linecap="round" '
     'stroke-linejoin="round"')


def s(w=2.2):
    return S % w


def icon(color, sil, det=""):
    shadow = "".join('<use xlink:href="#g" transform="translate(%g %g)"/>' % (i, i)
                     for i in range(16, 0, -1))
    return ('<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" '
            'width="32" height="32" viewBox="0 0 32 32"><defs><clipPath id="c">'
            '<circle cx="16" cy="16" r="15"/></clipPath><g id="g">%s</g></defs>'
            '<circle cx="16" cy="16" r="15" fill="%s"/>'
            '<g clip-path="url(#c)" color="#000" opacity=".2">%s</g>'
            '<use xlink:href="#g" color="#fff"/>%s</svg>' % (sil, color, shadow, det.replace("C", color)
                                                             if det else ""))


def page(x, y, w, h):
    return '<path d="M%g %gh%gl3.5 3.5v%gh-%gz" %s/>' % (x, y, w - 3.5, h - 3.5, w, F)


def page_edge(x, y, w, h, color="C", width=1.3):
    return ('<path d="M%g %gh%gl3.5 3.5v%gh-%gz" fill="none" stroke="%s" stroke-width="%g" '
            'stroke-linejoin="round"/>' % (x, y, w - 3.5, h - 3.5, w, color, width))


def lines(x, y, w, n, step=2.6, color="C"):
    return "".join('<rect x="%g" y="%g" width="%g" height="1.3" rx=".6" fill="%s"/>'
                   % (x, y + i * step, w if i < n - 1 else w * .6, color) for i in range(n))


def plus(cx, cy, r=3, w=2, color="C"):
    return ('<path d="M%g %gv%gM%g %gh%g" stroke="%s" stroke-width="%g" stroke-linecap="round"/>'
            % (cx, cy - r, 2 * r, cx - r, cy, 2 * r, color, w))


def window(inner=""):
    sil = '<rect x="7" y="8" width="18" height="16" rx="2.2" %s/>' % F
    det = '<rect x="8.7" y="12" width="14.6" height="10.3" rx=".8" fill="C"/>' + inner
    return sil, det


def gear():
    pts = []
    for i in range(32):
        a = math.pi * 2 * i / 32
        r = 8.2 if (i // 2) % 2 == 0 else 6.2
        pts.append("%.2f %.2f" % (16 + r * math.cos(a), 16 + r * math.sin(a)))
    return '<path d="M%sz" %s/>' % ("L".join(pts), F)


def arrow_out(y=16):
    return '<path d="M13 %gH24.5M21.5 %gl3 3-3 3" %s/>' % (y, y - 3, s(2.2))


def make():
    w = {}
    w["close"] = (RED, '<path d="M11 11l10 10M21 11l-10 10" %s/>' % s(2.8), "")
    w["delete"] = (RED, '<rect x="8.5" y="9" width="15" height="2.4" rx="1.1" %s/>'
                   '<rect x="13" y="6.8" width="6" height="3" rx="1" %s/>'
                   '<path d="M10 12.5h12l-1.1 12h-9.8z" %s/>' % (F, F, F),
                   "".join('<rect x="%g" y="14.5" width="1.3" height="8" rx=".6" fill="C"/>' % x
                           for x in (13.2, 15.35, 17.5)))
    w["copy"] = (ORANGE, page(9, 7, 11, 14) + page(13, 11, 11, 14),
                 page_edge(13, 11, 11, 14, width=1.6) + lines(15.3, 16.2, 6.4, 3))
    w["paste"] = (AMBER, '<rect x="8" y="9" width="12" height="15" rx="1.8" %s/>'
                  '<rect x="11" y="6.6" width="6" height="3.6" rx="1.2" %s/>' % (F, F)
                  + page(14, 13, 10, 13),
                  '<rect x="12.4" y="8" width="3.2" height="1.3" rx=".6" fill="C"/>'
                  + page_edge(14, 13, 10, 13, width=1.6) + lines(16, 18, 5.6, 3))
    w["cut"] = (TEAL, '<circle cx="11.8" cy="21" r="3" %s/><circle cx="20.2" cy="21" r="3" %s/>'
                '<path d="M13.6 18.6L21 7.5M18.4 18.6L11 7.5" %s/>' % (s(2), s(2), s(2.2)), "")
    folder = ('<path d="M7.5 10.2a1.2 1.2 0 0 1 1.2-1.2h4.6l2 2.2h7.9a1.2 1.2 0 0 1 1.2 1.2v10.4'
              'a1.2 1.2 0 0 1-1.2 1.2H8.7a1.2 1.2 0 0 1-1.2-1.2z" %s/>' % F)
    w["new-folder"] = (AMBER, folder, '<path d="M8 13.4h16" stroke="C" stroke-width="1.2"/>'
                       + plus(16, 18.4, 2.8, 2.1))
    w["open"] = (RED, '<path d="M8 10.2a1.2 1.2 0 0 1 1.2-1.2h4.4l2 2.2h6.7a1.2 1.2 0 0 1 1.2 1.2'
                 'v2.2H8z" %s/><path d="M7 15.6h18.5l-2.4 8.4H9.3z" %s/>' % (F, F),
                 '<path d="M7.4 15.6h18" stroke="C" stroke-width="1.3"/>')
    w["move-to"] = (BLUE, page(8, 7, 11, 18) + arrow_out(),
                    '<path d="M13 16h6" stroke="C" stroke-width="2.2" stroke-linecap="round"/>')
    w["copy-to"] = (GREEN, page(6.5, 6, 10, 15) + page(9.5, 9, 10, 16) + arrow_out(17),
                    page_edge(9.5, 9, 10, 16, width=1.5)
                    + '<path d="M13 17h6.5" stroke="C" stroke-width="2.2" stroke-linecap="round"/>')
    w["new-item"] = (GREEN, page(10, 7, 12, 18), plus(16, 17.2, 3, 2.1))
    w["link"] = (ORANGE, '<g transform="rotate(-45 16 16)"><rect x="6.5" y="12.8" width="10.5" '
                 'height="6.4" rx="3.2" %s/><rect x="15" y="12.8" width="10.5" height="6.4" rx="3.2" '
                 '%s/></g>' % (s(2.3), s(2.3)), "")
    w["pin"] = (RED, '<path d="M12.5 7h7v2.2h-1.2v4.3l3 3V18h-10.6v-1.5l3-3V9.2h-1.2z" %s/>'
                '<path d="M16 18v7" %s/>' % (F, s(1.9)), "")
    w["properties"] = (ORANGE, '<circle cx="16" cy="16" r="8.5" %s/>' % F,
                       '<circle cx="16" cy="11.6" r="1.6" fill="C"/>'
                       '<rect x="14.6" y="14.4" width="2.8" height="8" rx="1.2" fill="C"/>')
    w["options"] = (LBLUE, gear(), '<circle cx="16" cy="16" r="3" fill="C"/>')
    w["rename"] = (PURPLE, '<rect x="6.5" y="11" width="16" height="10" rx="2" %s/>'
                   '<path d="M23 8v16M20.8 8h4.4M20.8 24h4.4" %s/>' % (s(2), s(1.9)),
                   "")
    w["rename"] = (PURPLE, w["rename"][1] + '<rect x="9.5" y="14.8" width="7" height="2.4" rx="1.1" '
                   '%s/>' % F, "")
    box = '<rect x="8" y="8" width="16" height="16" rx="2" %s stroke-dasharray="3 2.4"/>' % s(1.8)
    w["select-all"] = (GREEN, box + '<path d="M12 16.3l3 3 5.2-6.4" %s/>' % s(2.4), "")
    w["select-none"] = (GREY, box + '<path d="M13 13l6 6M19 13l-6 6" %s/>' % s(2.2), "")
    w["invert"] = (PURPLE, box + '<path d="M10.5 10.5h11l-11 11z" %s/>' % F, "")
    w["large-icons"] = (BLUE, "".join('<rect x="%g" y="%g" width="6.8" height="6.8" rx="1.6" %s/>'
                                      % (x, y, F) for x in (8.4, 16.8) for y in (8.4, 16.8)), "")
    w["medium-icons"] = (LBLUE, "".join('<rect x="%g" y="%g" width="4.2" height="4.2" rx="1" %s/>'
                                        % (8.6 + c * 5.3, 8.6 + r * 5.3, F)
                                        for r in range(3) for c in range(3)), "")
    w["details-view"] = (GREY, "".join('<rect x="8" y="%g" width="3" height="3" rx=".8" %s/>'
                                       '<rect x="12.6" y="%g" width="11.4" height="2.4" rx="1.1" %s/>'
                                       % (8.4 + i * 4.3, F, 8.7 + i * 4.3, F) for i in range(4)), "")
    w["details-pane"] = (PINK,) + window("".join(
        '<rect x="16.5" y="%g" width="5" height="1.4" rx=".6" fill="#fff"/>' % (14 + i * 2.6)
        for i in range(3)) + '<rect x="15" y="12" width="1.2" height="10.3" fill="#fff"/>')
    w["nav-pane"] = (BLUE,) + window('<rect x="10" y="13.4" width="4" height="7.5" rx=".6" '
                                     'fill="#fff"/>')
    w["preview-pane"] = (ORANGE,) + window('<rect x="16.4" y="13.4" width="5.6" height="7.5" '
                                           'rx=".6" fill="#fff"/>')
    w["split"] = (BLUE,) + window('<rect x="10" y="13.4" width="5.3" height="7.5" rx=".6" '
                                  'fill="#fff"/><rect x="16.7" y="13.4" width="5.3" height="7.5" '
                                  'rx=".6" fill="#fff"/>')
    w["window-new"] = (PINK,) + window(plus(16, 17.2, 2.8, 2.1, "#fff"))
    w["open-with"] = (LBLUE, '<path d="M19 8.5h-8a2.2 2.2 0 0 0-2.2 2.2v10.6a2.2 2.2 0 0 0 2.2 2.2'
                      'h10.6a2.2 2.2 0 0 0 2.2-2.2v-8" %s/><path d="M15.5 16.5L24 8M19 8h5v5" %s/>'
                      % (s(2.1), s(2.1)), "")
    w["terminal"] = (DARK, '<rect x="7" y="9" width="18" height="14" rx="2.2" %s/>' % F,
                     '<path d="M10.5 13l3 2.6-3 2.6" fill="none" stroke="C" stroke-width="1.9" '
                     'stroke-linecap="round" stroke-linejoin="round"/>'
                     '<rect x="15" y="18.4" width="6" height="1.8" rx=".9" fill="C"/>')
    finder = "".join('<rect x="%g" y="%g" width="6.6" height="6.6" rx="1.2" %s/>' % (x, y, F)
                     for x, y in ((8, 8), (17.4, 8), (8, 17.4)))
    dots = "".join('<rect x="%g" y="%g" width="2.6" height="2.6" rx=".5" %s/>' % (x, y, F)
                   for x, y in ((17.6, 17.6), (21.4, 21.4), (21.4, 17.6), (17.6, 21.4)))
    w["qr"] = (DARK, finder + dots, "".join(
        '<rect x="%g" y="%g" width="3.6" height="3.6" rx=".6" fill="none" stroke="C" '
        'stroke-width="1.1"/>' % (x + 1.5, y + 1.5) for x, y in ((8, 8), (17.4, 8), (8, 17.4))))
    w["send"] = (LBLUE, '<path d="M7 15.6L25 8l-6 17-3.4-7.6z" %s/>' % F,
                 '<path d="M15.6 17.4L25 8" stroke="C" stroke-width="1.3"/>')
    w["shortcut"] = (AMBER, '<path d="M8.5 23c0-6 4-9.8 10-9.8h.8V9l6.2 6.6-6.2 6.6v-4.3h-.8'
                     'c-4.2 0-7.2 1.6-10 5.1z" %s/>' % F, "")
    w["sort"] = (TEAL, '<path d="M12 24V8M8.2 11.8L12 8l3.8 3.8M20 8v16M16.2 20.2L20 24l3.8-3.8" '
                 '%s/>' % s(2.2), "")
    w["wifi"] = (TEAL, '<path d="M8 15.4a11.4 11.4 0 0 1 16 0M10.9 18.7a7.2 7.2 0 0 1 10.2 0" %s/>'
                 '<circle cx="16" cy="22.4" r="1.9" %s/>' % (s(2.3), F), "")
    w["usb"] = (BLUE, '<path d="M16 9.5v13M16 19.5l-4.6-3.2v-2.6M16 17.6l4.6-2.8v-2.4" %s/>'
                '<path d="M16 6.5l2.6 3.7h-5.2z" %s/><circle cx="11.4" cy="12.4" r="1.7" %s/>'
                '<rect x="19" y="9.4" width="3.2" height="3.2" rx=".6" %s/>'
                '<circle cx="16" cy="23.4" r="2.1" %s/>' % (s(2.1), F, F, F, F), "")
    w["zip"] = (AMBER, '<rect x="8" y="7.5" width="16" height="17" rx="2" %s/>' % F,
                "".join('<rect x="%g" y="%g" width="2" height="1.6" fill="C"/>'
                        % (15 if i % 2 else 17, 7.5 + i * 1.6) for i in range(6))
                + '<rect x="14.4" y="17.4" width="3.2" height="4.4" rx="1" fill="none" stroke="C" '
                'stroke-width="1.2"/>')
    w["edit"] = (ORANGE, '<g transform="rotate(45 16 16)"><rect x="13.4" y="4.6" width="5.2" '
                 'height="17.6" rx="1.2" %s/><path d="M13.4 23.4h5.2l-2.6 4.6z" %s/></g>' % (F, F),
                 '<g transform="rotate(45 16 16)"><rect x="13.4" y="8.2" width="5.2" height="1.3" '
                 'fill="C"/></g>')
    return {k: icon(*v) for k, v in w.items()}


def main():
    os.makedirs(OUT, exist_ok=True)
    icons = make()
    for name, text in icons.items():
        with open(os.path.join(OUT, "harbor-rbc-%s.svg" % name), "w") as f:
            f.write(text + "\n")
    print("wrote %d icons to %s" % (len(icons), OUT))


if __name__ == "__main__":
    main()
