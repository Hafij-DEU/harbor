#!/usr/bin/env python3
"""Build the Harbor Files app icon from design/app-icon-source-v2.webp.

The picture is split into two layers so the folder tab can follow the accent
colour:
  * body  - the whole folder on a transparent background (white removed,
            the floor shadow kept as a soft black shadow)
  * tab   - the orange tab as a grey shade map (white = the tab's base
            colour) plus a white highlight layer
The SVG tints the shade map with feFlood + feBlend multiply, so another
accent is a one-word change (files_appicon.py does that at run time).

Writes data/icons/io.github.hafij_deu.Harbor.Files.svg (default orange),
design/app-icon-512.png and the Android launcher art. Needs Pillow + numpy
(build time only).

    python3 tools/make_app_icon.py
"""

import base64
import io
import os

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
SRC = os.path.join(ROOT, "design", "app-icon-source-v2.webp")
SVG_OUT = os.path.join(ROOT, "data", "icons", "io.github.hafij_deu.Harbor.Files.svg")
LAYERS = os.path.join(ROOT, "src", "harbor", "data")
ANDROID = os.path.join(ROOT, "android", "app", "src", "main", "res", "drawable-nodpi",
                       "ic_launcher_art.png")
SIZE = 512
ACCENT_TOKEN = "#HBACCENT"


def load():
    im = Image.open(SRC).convert("RGB")
    a = np.asarray(im).astype(np.float32) / 255.0
    mx, mn = a.max(2), a.min(2)
    sat = np.where(mx > 0, (mx - mn) / np.maximum(mx, 1e-6), 0)
    # background candidates: bright and grey; flood from the corners so the
    # silver inside the folder is never touched
    cand = ((sat < 0.10) & (mx > 0.62)).astype(np.uint8) * 255
    m = Image.fromarray(cand, "L").copy()      # a copy: floodfill on an array view is a no-op
    h, w = cand.shape
    for xy in ((0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)):
        if m.getpixel(xy) == 255:
            ImageDraw.floodfill(m, xy, 128)
    bg = np.asarray(m) == 128
    lum = a.mean(2)
    shadow = np.clip((0.985 - lum) * 2.4, 0, 0.55)
    alpha = np.where(bg, shadow, 1.0)
    rgb = np.where(bg[..., None], 0.0, a)
    # orange tab: hue 5..50 degrees, saturated, outside the background
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    d = np.maximum(mx - mn, 1e-6)
    hue = np.where(mx == r, ((g - b) / d) % 6, np.where(mx == g, (b - r) / d + 2, (r - g) / d + 4)) * 60
    tab = (~bg) & (sat > 0.18) & (mx > 0.35) & (hue > 5) & (hue < 50)
    # close the speckles (glossy highlights fall out of the hue test), then grow
    # one pixel over the anti-aliased edge
    tab_img = Image.fromarray(tab.astype(np.uint8) * 255, "L").copy()
    tab_img = tab_img.filter(ImageFilter.MaxFilter(9)).filter(ImageFilter.MinFilter(9))
    tab_img = tab_img.filter(ImageFilter.MaxFilter(3))
    tab = (np.asarray(tab_img) > 0) & (~bg | (mx < 0.97))
    tab &= alpha > 0.05
    # smooth the value channel so JPEG-ish noise does not turn into grain
    mx = np.asarray(Image.fromarray((mx * 255).astype(np.uint8), "L").copy()
                    .filter(ImageFilter.GaussianBlur(7))).astype(np.float32) / 255.0
    # square crop around the folder
    ys, xs = np.where(alpha > 0.04)
    y0, y1, x0, x1 = ys.min(), ys.max(), xs.min(), xs.max()
    side = int(max(y1 - y0, x1 - x0) * 1.16)
    cy, cx = (y0 + y1) // 2, (x0 + x1) // 2
    box = (cx - side // 2, cy - side // 2, cx - side // 2 + side, cy - side // 2 + side)
    return rgb, alpha, tab, mx, box


def crop(arr, box):
    """Crop with zero padding where the box leaves the picture."""
    x0, y0, x1, y1 = box
    h, w = arr.shape[:2]
    out = np.zeros((y1 - y0, x1 - x0) + arr.shape[2:], arr.dtype)
    sx0, sy0, sx1, sy1 = max(0, x0), max(0, y0), min(w, x1), min(h, y1)
    out[sy0 - y0:sy1 - y0, sx0 - x0:sx1 - x0] = arr[sy0:sy1, sx0:sx1]
    return out


def rgba_image(rgb, alpha):
    data = np.dstack([rgb, alpha])
    return Image.fromarray((np.clip(data, 0, 1) * 255 + 0.5).astype(np.uint8), "RGBA")


def png_b64(im):
    buf = io.BytesIO()
    im.save(buf, "PNG", optimize=True)
    return base64.b64encode(buf.getvalue()).decode()


def main():
    rgb, alpha, tab, val, box = load()
    rgb, alpha, tab, val = (crop(x, box) for x in (rgb, alpha, tab, val))
    body = rgba_image(rgb, alpha).resize((SIZE, SIZE), Image.LANCZOS)
    base = float(np.median(val[tab]))
    base_rgb = np.median(rgb[tab], axis=0)
    shade = np.clip(val / base, 0, 1)
    hi = np.clip((val - base) / max(1e-3, 1 - base), 0, 1) * 0.35
    ta = tab.astype(np.float32) * alpha
    shade_im = rgba_image(np.dstack([shade] * 3), ta).resize((SIZE, SIZE), Image.LANCZOS)
    hi_im = rgba_image(np.ones(rgb.shape, np.float32), hi * ta).resize((SIZE, SIZE), Image.LANCZOS)
    default = "#%02x%02x%02x" % tuple(int(c * 255 + 0.5) for c in base_rgb)

    svg = (
        '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" '
        'width="{s}" height="{s}" viewBox="0 0 {s} {s}">'
        '<defs><filter id="tint" x="0" y="0" width="1" height="1" '
        'color-interpolation-filters="sRGB">'
        '<feFlood flood-color="{tok}" result="c"/>'
        '<feComposite in="c" in2="SourceAlpha" operator="in" result="ca"/>'
        '<feBlend in="ca" in2="SourceGraphic" mode="multiply"/></filter></defs>'
        '<image width="{s}" height="{s}" xlink:href="data:image/png;base64,{body}"/>'
        '<image width="{s}" height="{s}" filter="url(#tint)" '
        'xlink:href="data:image/png;base64,{shade}"/>'
        '<image width="{s}" height="{s}" xlink:href="data:image/png;base64,{hi}"/>'
        '</svg>\n').format(s=SIZE, tok=ACCENT_TOKEN, body=png_b64(body), shade=png_b64(shade_im),
                           hi=png_b64(hi_im))
    os.makedirs(LAYERS, exist_ok=True)
    with open(os.path.join(LAYERS, "app-icon-template.svg"), "w") as f:
        f.write(svg)
    with open(os.path.join(LAYERS, "app-icon-default.txt"), "w") as f:
        f.write(default + "\n")
    with open(SVG_OUT, "w") as f:
        f.write(svg.replace(ACCENT_TOKEN, default))

    # flat PNG of the default look (README, Android)
    tint = np.dstack([shade] * 3) * base_rgb
    flat = rgb.copy()
    flat[tab] = tint[tab]
    flat = flat * (1 - hi[..., None] * tab[..., None]) + hi[..., None] * tab[..., None]
    png = rgba_image(flat, alpha).resize((SIZE, SIZE), Image.LANCZOS)
    png.save(os.path.join(ROOT, "design", "app-icon-512.png"))
    # Android adaptive background: soft white tile, art inside the safe zone
    tile = Image.new("RGBA", (432, 432), (244, 246, 250, 255))
    art = png.resize((262, 262), Image.LANCZOS)
    tile.alpha_composite(art, ((432 - 262) // 2, (432 - 262) // 2 + 6))
    tile.convert("RGB").save(ANDROID, optimize=True)
    print("default tab colour", default, "svg", os.path.getsize(SVG_OUT) // 1024, "KB")


if __name__ == "__main__":
    main()
