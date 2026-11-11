#!/usr/bin/env python3
"""Generate Harbor's own icon set (data/icons/hicolor/scalable/...).

Original drawings in a soft, Fluent-inspired style: gradients, rounded
corners, a coloured emblem chip on folders and a coloured mark on files.
Nothing here is copied from another icon set. Run it again after changing a
shape; the SVGs are checked in.

    python3 tools/make_icons.py
"""

import math
import os

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "data", "icons", "hicolor",
                    "scalable")


def svg(body, defs=""):
    return ('<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64">'
            "<defs>%s</defs>%s</svg>\n" % (defs, body))


def lin(id_, c1, c2, x1=0, y1=0, x2=0, y2=1):
    return ('<linearGradient id="%s" x1="%s" y1="%s" x2="%s" y2="%s">'
            '<stop offset="0" stop-color="%s"/><stop offset="1" stop-color="%s"/>'
            "</linearGradient>" % (id_, x1, y1, x2, y2, c1, c2))


def rad(id_, c1, c2, cx=0.35, cy=0.3, r=0.8):
    return ('<radialGradient id="%s" cx="%s" cy="%s" r="%s">'
            '<stop offset="0" stop-color="%s"/><stop offset="1" stop-color="%s"/>'
            "</radialGradient>" % (id_, cx, cy, r, c1, c2))


W = 'fill="none" stroke="#fff" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"'

# ------------------------------------------------------------------ folders

FOLDER_DEFS = (lin("back", "#efae38", "#dc9420") + lin("front", "#ffdc7a", "#f7bd3c")
               + lin("shade", "#000", "#000"))
FOLDER_BACK = ('<path d="M6 14a3 3 0 0 1 3-3h14.2a3 3 0 0 1 2.1.9L29.4 16H55a3 3 0 0 1 3 3v33'
               'a3 3 0 0 1-3 3H9a3 3 0 0 1-3-3z" fill="url(#back)"/>')
FOLDER_PAPER = '<rect x="10" y="17" width="44" height="12" rx="1.5" fill="#fffdf6" opacity=".95"/>'
FOLDER_FRONT = ('<rect x="6" y="21" width="52" height="34" rx="3" fill="url(#front)"/>'
                '<path d="M9 21.6h46" stroke="#fff3c4" stroke-width="1.2" stroke-linecap="round"/>'
                '<rect x="6" y="53" width="52" height="2" rx="1" fill="#c98a12" opacity=".35"/>')

# glyphs drawn in white on a chip centred at (32, 40)
EMBLEMS = {
    "documents": ("#2b7cd3", '<path %s d="M27.5 36h9M27.5 39.5h9M27.5 43h5.5"/>' % W),
    "downloads": ("#1e9e5a", '<path %s d="M32 34.8v7.6M28.6 39l3.4 3.4 3.4-3.4M27.8 45h8.4"/>' % W),
    "music": ("#e2523a", '<path %s d="M33 43V35.2q3.4.6 4.4 3"/><circle cx="30.6" cy="43.2" r="2.3" '
              'fill="#fff"/>' % W),
    "pictures": ("#13979f", '<path d="M26.6 44.6l4-5 2.9 3.4 2.1-2.2 3.6 3.8z" fill="#fff"/>'
                 '<circle cx="35.6" cy="36.6" r="1.6" fill="#fff"/>'),
    "videos": ("#7e57c2", '<path d="M30 35.4v9.2l7.4-4.6z" fill="#fff" stroke="#fff" '
               'stroke-width="1" stroke-linejoin="round"/>'),
    "desktop": ("#2a8be0", '<rect x="26.3" y="34.6" width="11.4" height="7.6" rx="1.2" %s/>'
                '<path %s d="M30 45.4h4M32 42.4v3"/>' % (W, W)),
    "home": ("#0f7b6c", '<path %s d="M26.6 40.4L32 35.4l5.4 5M28.2 39.4v5.6h7.6v-5.6"/>' % W),
    "public": ("#3949ab", '<circle cx="29.4" cy="37.4" r="1.9" fill="#fff"/><circle cx="34.8" '
               'cy="37.4" r="1.9" fill="#fff"/><path %s d="M26.6 44.6a2.9 2.9 0 0 1 5.6 0'
               'M32 44.6a2.9 2.9 0 0 1 5.6 0"/>' % W),
    "templates": ("#546e7a", '<rect x="26.6" y="35" width="10.8" height="9.6" rx="1.2" %s/>'
                  '<path %s d="M26.6 38.4h10.8M30.4 38.4v6.2"/>' % (W, W)),
}


def folder(emblem=None):
    body = FOLDER_BACK + FOLDER_PAPER + FOLDER_FRONT
    if emblem:
        color, glyph = EMBLEMS[emblem]
        body += ('<rect x="22" y="31.6" width="20" height="16.8" rx="4.2" fill="%s"/>'
                 '<rect x="22" y="31.6" width="20" height="8" rx="4" fill="#fff" opacity=".12"/>'
                 % color) + glyph
    return svg(body, FOLDER_DEFS)


# ------------------------------------------------------------------ files

PAGE_DEFS = lin("page", "#ffffff", "#eef1f5") + lin("fold", "#e9ecf1", "#cfd5dd")
PAGE = ('<path d="M17 5h20.8a3 3 0 0 1 2.1.9l11.2 11.2a3 3 0 0 1 .9 2.1V56a3 3 0 0 1-3 3H17'
        'a3 3 0 0 1-3-3V8a3 3 0 0 1 3-3z" fill="url(#page)" stroke="#c3c9d2" stroke-width="1"/>'
        '<path d="M38 5.4V15a3 3 0 0 0 3 3h9.6" fill="url(#fold)" stroke="#c3c9d2" '
        'stroke-width="1" stroke-linejoin="round"/>')
LINES = ('<path d="M21 %d h%d" stroke="%s" stroke-width="2" stroke-linecap="round"/>')


def lines(ys, color="#a3abb7", width=24, last=14):
    out = ""
    for i, y in enumerate(ys):
        out += LINES % (y, (last if i == len(ys) - 1 else width), color)
    return out


def tab_label(color, glyph):
    """Coloured label sticking out of the left edge with a white glyph."""
    return ('<rect x="8" y="35" width="30" height="17" rx="3.2" fill="%s"/>'
            '<rect x="8" y="35" width="30" height="7" rx="3" fill="#fff" opacity=".14"/>%s'
            % (color, glyph))


def file_icon(kind=None):
    defs = PAGE_DEFS
    mark = ""
    if kind == "text":
        mark = lines((26, 31, 36, 41, 46))
    elif kind == "code":
        mark = ('<path d="M26.5 31.5l-6 6 6 6M39.5 31.5l6 6-6 6M35 29l-4 17" fill="none" '
                'stroke="#2b7cd3" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>')
    elif kind == "image":
        defs += lin("sky", "#6fd0fb", "#1f8fd6") + lin("hill", "#5cc06a", "#2e8b3a")
        mark = ('<clipPath id="ic"><rect x="19" y="25" width="28" height="24" rx="3"/></clipPath>'
                '<g clip-path="url(#ic)"><rect x="19" y="25" width="28" height="24" fill="url(#sky)"/>'
                '<circle cx="39.5" cy="32" r="3.2" fill="#ffd54f"/>'
                '<path d="M17 51l10-13 7 8 4-4 11 9z" fill="url(#hill)"/></g>')
    elif kind == "audio":
        defs += lin("aud", "#ff9a3d", "#e8590c")
        mark = ('<circle cx="33" cy="39" r="11.5" fill="url(#aud)"/>'
                '<path d="M34.6 44.4V32.2q4.4.9 5.6 4" fill="none" stroke="#fff" stroke-width="2.2"'
                ' stroke-linecap="round" stroke-linejoin="round"/>'
                '<circle cx="31.3" cy="44.6" r="3" fill="#fff"/>')
    elif kind == "video":
        defs += lin("vid", "#9575cd", "#5e35b1")
        mark = ('<rect x="19" y="28" width="28" height="21" rx="4" fill="url(#vid)"/>'
                '<path d="M30.2 33v11l9-5.5z" fill="#fff" stroke="#fff" stroke-width="1.2" '
                'stroke-linejoin="round"/>')
    elif kind == "archive":
        teeth = "".join('<rect x="%s" y="%d" width="3" height="2.6" rx=".6" fill="#8d6e63"/>'
                        % (30 if i % 2 else 33, 7 + i * 3) for i in range(8))
        mark = (teeth + '<rect x="28.5" y="31" width="9" height="12" rx="2.4" fill="#6d4c41"/>'
                '<rect x="31" y="35.6" width="4" height="4.6" rx="1" fill="#d7ccc8"/>')
    elif kind == "pdf":
        glyph = ('<path d="M14.4 48.4v-9.4h3.2a2.5 2.5 0 0 1 0 5h-3.2M21.6 39v9.4h1.7'
                 'c4.6 0 4.6-9.4 0-9.4zM30.6 48.4V39h5M30.6 43.4h4" fill="none" stroke="#fff" '
                 'stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>')
        mark = lines((22, 27)) + tab_label("#d93838", glyph)
    elif kind == "document":
        glyph = ('<path d="M14 40h17M14 43.5h17M14 47h11" stroke="#fff" stroke-width="1.8" '
                 'stroke-linecap="round"/>')
        mark = lines((22, 27)) + tab_label("#2b6cc4", glyph)
    elif kind == "spreadsheet":
        glyph = ('<rect x="14" y="39" width="18" height="10" rx="1" fill="none" stroke="#fff" '
                 'stroke-width="1.6"/><path d="M14 44h18M20 39v10M26 39v10" stroke="#fff" '
                 'stroke-width="1.6"/>')
        mark = lines((22, 27)) + tab_label("#1e8e3e", glyph)
    elif kind == "presentation":
        glyph = ('<path d="M15 48.6h16" stroke="#fff" stroke-width="1.6" stroke-linecap="round"/>'
                 '<rect x="16" y="43" width="3.4" height="4.4" rx=".6" fill="#fff"/>'
                 '<rect x="21.3" y="39.6" width="3.4" height="7.8" rx=".6" fill="#fff"/>'
                 '<rect x="26.6" y="41.6" width="3.4" height="5.8" rx=".6" fill="#fff"/>')
        mark = lines((22, 27)) + tab_label("#e8590c", glyph)
    elif kind == "executable":
        defs += lin("term", "#455a64", "#263238")
        mark = ('<rect x="18.5" y="27" width="29" height="21" rx="3.4" fill="url(#term)"/>'
                '<path d="M23.4 33l4.4 4-4.4 4" fill="none" stroke="#69f0ae" stroke-width="2.2" '
                'stroke-linecap="round" stroke-linejoin="round"/>'
                '<path d="M30.6 42.4h8" stroke="#eceff1" stroke-width="2.2" stroke-linecap="round"/>')
    elif kind == "diskimage":
        defs += rad("disc", "#ffffff", "#b4c0c8", .4, .35, .75)
        mark = ('<circle cx="33" cy="38.5" r="12.5" fill="url(#disc)" stroke="#8fa1ad"/>'
                '<path d="M25.4 33.6a9.4 9.4 0 0 1 6-4.4" fill="none" stroke="#fff" stroke-width="2"'
                ' stroke-linecap="round" opacity=".9"/>'
                '<circle cx="33" cy="38.5" r="3.6" fill="#eef1f5" stroke="#8fa1ad"/>')
    elif kind == "font":
        mark = ('<path d="M23.5 48.5L33 26.5l9.5 22M27 41h12" fill="none" stroke="#3f51b5" '
                'stroke-width="3.4" stroke-linecap="round" stroke-linejoin="round"/>')
    elif kind == "web":
        defs += lin("globe", "#e3f2fd", "#bbdefb")
        mark = ('<circle cx="33" cy="38.5" r="12" fill="url(#globe)" stroke="#1e88e5" '
                'stroke-width="2"/><ellipse cx="33" cy="38.5" rx="5" ry="12" fill="none" '
                'stroke="#1e88e5" stroke-width="1.6"/><path d="M21 38.5h24M23 32.5h20M23 44.5h20" '
                'stroke="#1e88e5" stroke-width="1.6"/>')
    return svg(PAGE + mark, defs)


# ------------------------------------------------------------------ places


def star_path(cx, cy, ro, ri):
    pts = []
    for i in range(10):
        r = ro if i % 2 == 0 else ri
        a = -math.pi / 2 + i * math.pi / 5
        pts.append("%.2f %.2f" % (cx + r * math.cos(a), cy + r * math.sin(a)))
    return "M" + "L".join(pts) + "z"


def place(kind):
    if kind == "drive":
        defs = lin("dr", "#f5f7f8", "#b8c4cb") + lin("drf", "#9aaab4", "#7d8f9a")
        body = ('<rect x="5" y="20" width="54" height="26" rx="5" fill="url(#dr)" stroke="#7f919c"/>'
                '<path d="M5.5 36h53v5a4.5 4.5 0 0 1-4.5 4.5H10A4.5 4.5 0 0 1 5.5 41z" fill="url(#drf)"/>'
                '<rect x="46" y="39.6" width="7" height="2.4" rx="1.2" fill="#3ddc84"/>'
                '<path d="M10 24.5h44" stroke="#fff" stroke-width="1.2" stroke-linecap="round" '
                'opacity=".8"/>')
    elif kind == "usb":
        defs = lin("usb", "#6f7fd6", "#3a4aa8")
        body = ('<rect x="40" y="25" width="18" height="14" rx="1.6" fill="#dfe5e8" stroke="#8fa1ad"/>'
                '<rect x="46" y="29" width="3" height="3" fill="#8fa1ad"/>'
                '<rect x="51" y="29" width="3" height="3" fill="#8fa1ad"/>'
                '<rect x="6" y="21" width="37" height="22" rx="5" fill="url(#usb)"/>'
                '<path d="M11 25.5h27" stroke="#fff" stroke-width="1.2" stroke-linecap="round" '
                'opacity=".35"/><circle cx="14" cy="36" r="2" fill="#69f0ae"/>')
    elif kind == "optical":
        defs = (rad("cd", "#ffffff", "#b9c4cc", .38, .32, .8)
                + lin("sheen", "#ff8a80", "#82b1ff", 0, 0, 1, 1))
        body = ('<circle cx="32" cy="32" r="26" fill="url(#cd)" stroke="#8fa1ad"/>'
                '<circle cx="32" cy="32" r="26" fill="url(#sheen)" opacity=".22"/>'
                '<path d="M15 24a19 19 0 0 1 11-10.5" fill="none" stroke="#fff" stroke-width="3" '
                'stroke-linecap="round" opacity=".9"/>'
                '<circle cx="32" cy="32" r="8" fill="#e6eaee" stroke="#8fa1ad"/>'
                '<circle cx="32" cy="32" r="3.4" fill="#fff" stroke="#8fa1ad"/>')
    elif kind == "network":
        defs = lin("scr", "#5ab8f5", "#1565c0")
        body = ('<path d="M32 33v10M10 46h44" stroke="#607d8b" stroke-width="3" '
                'stroke-linecap="round"/>'
                '<rect x="16" y="6" width="32" height="24" rx="3.4" fill="#37474f"/>'
                '<rect x="18.6" y="8.6" width="26.8" height="18.8" rx="1.8" fill="url(#scr)"/>'
                '<rect x="27" y="30" width="10" height="4" rx="1" fill="#90a4ae"/>'
                '<rect x="4" y="41" width="12" height="11" rx="2.4" fill="#78909c"/>'
                '<rect x="48" y="41" width="12" height="11" rx="2.4" fill="#78909c"/>'
                '<rect x="26" y="41" width="12" height="11" rx="2.4" fill="#546e7a"/>'
                '<circle cx="32" cy="46.5" r="1.8" fill="#69f0ae"/>')
    elif kind == "thispc":
        defs = lin("scr", "#6fd0fb", "#1976d2") + lin("st", "#cfd8dc", "#90a4ae")
        body = ('<path d="M26 45h12l2.4 8H23.6z" fill="url(#st)"/>'
                '<rect x="19" y="52" width="26" height="4.4" rx="2.2" fill="#b0bec5"/>'
                '<rect x="4" y="8" width="56" height="38" rx="4.6" fill="#37474f"/>'
                '<rect x="7.4" y="11.4" width="49.2" height="31.2" rx="2.4" fill="url(#scr)"/>'
                '<path d="M7.4 36L36 11.4h20.6v4L14 42.6H9.8a2.4 2.4 0 0 1-2.4-2.4z" fill="#fff" '
                'opacity=".12"/>')
    elif kind in ("trash", "trash-full"):
        defs = lin("bin", "#eef6fc", "#b7d3e6") + lin("lid", "#a9bcc8", "#7f95a3")
        papers = ""
        lid = '<rect x="11" y="14" width="42" height="6.4" rx="2.4" fill="url(#lid)"/>' \
              '<rect x="25" y="9.4" width="14" height="6" rx="2.2" fill="none" stroke="#7f95a3" ' \
              'stroke-width="2"/>'
        if kind == "trash-full":
            papers = ('<rect x="19" y="13" width="14" height="17" rx="1.6" fill="#fff" stroke="#b0bec5"'
                      ' transform="rotate(-14 26 21)"/>'
                      '<rect x="31" y="11" width="14" height="17" rx="1.6" fill="#fffde7" '
                      'stroke="#c9b458" transform="rotate(12 38 19)"/>'
                      '<path d="M22 18h7M22 21h7" stroke="#b0bec5" stroke-width="1.4" '
                      'transform="rotate(-14 26 21)"/>')
            lid = ('<g transform="rotate(-16 11 16)">%s</g>' % lid)
        body = (papers + '<path d="M15 21h34l-3.2 33.2a3.4 3.4 0 0 1-3.4 3.1H21.6a3.4 3.4 0 0 1-3.4-3.1z" '
                'fill="url(#bin)" stroke="#7f95a3" stroke-width="1.4" stroke-linejoin="round"/>'
                '<path d="M25 27l.8 24M32 27v24M39 27l-.8 24" stroke="#8fa9ba" stroke-width="2" '
                'stroke-linecap="round"/>' + lid)
    elif kind == "quick-access":
        defs = lin("star", "#64c8fa", "#1e6fd9")
        body = ('<path d="%s" fill="url(#star)" stroke="#1a5fbf" stroke-width="1.2" '
                'stroke-linejoin="round"/>' % star_path(32, 34, 27, 11.5))
    else:
        raise ValueError(kind)
    return svg(body, defs)


def main():
    out = {}
    out[("places", "harbor-folder")] = folder()
    for key in EMBLEMS:
        out[("places", "harbor-folder-" + key)] = folder(key)
    out[("mimetypes", "harbor-file")] = file_icon()
    for k in ("text", "code", "image", "audio", "video", "archive", "pdf", "document",
              "spreadsheet", "presentation", "executable", "diskimage", "font", "web"):
        out[("mimetypes", "harbor-file-" + k)] = file_icon(k)
    for k in ("drive", "usb", "optical"):
        out[("devices", "harbor-drive" + ("" if k == "drive" else "-" + k))] = place(k)
    for k in ("network", "thispc", "trash", "trash-full", "quick-access"):
        out[("places", "harbor-" + k)] = place(k)
    for (ctx, name), text in out.items():
        d = os.path.join(ROOT, ctx)
        os.makedirs(d, exist_ok=True)
        with open(os.path.join(d, name + ".svg"), "w", encoding="utf-8") as f:
            f.write(text)
    print("wrote %d icons to %s" % (len(out), os.path.normpath(ROOT)))


if __name__ == "__main__":
    main()
