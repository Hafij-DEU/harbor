"""Glossy folder / file icons (Lumina Glass look, Icons > Glossy)."""

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "src"))

from harbor import files_icons as fi  # noqa: E402
from harbor import files_tint as tint  # noqa: E402

ICONS = os.path.join(os.path.dirname(__file__), "..", "data", "icons", "hicolor", "scalable")


def _exists(name):
    return any(os.path.exists(os.path.join(ICONS, ctx, name + ".svg"))
               for ctx in ("places", "mimetypes", "devices"))


def test_every_folder_and_file_icon_has_a_glossy_twin(monkeypatch):
    monkeypatch.setattr(fi, "GLOSSY", True)
    monkeypatch.setattr(fi, "MODE", "modern")
    for name in fi.ALL_ICONS:
        g = fi.gloss(name)
        if name.startswith(("harbor-folder", "harbor-file", "harbor-ext-")):
            assert g == name + "-glossy" and _exists(g), name
        else:
            assert g == name     # drives, trash, network keep the normal icon


def test_gloss_off_and_system_mode_keep_names(monkeypatch):
    monkeypatch.setattr(fi, "GLOSSY", False)
    assert fi.gloss("harbor-folder") == "harbor-folder"
    monkeypatch.setattr(fi, "GLOSSY", True)
    monkeypatch.setattr(fi, "MODE", "system")
    assert fi.gloss("harbor-folder") == "harbor-folder"


def test_glossy_folder_still_takes_folder_colours():
    with open(os.path.join(ICONS, "places", "harbor-folder-glossy.svg"), encoding="utf-8") as f:
        svg = f.read()
    blue = tint.recolor_svg(svg, "#3b82f6")
    assert tint.FRONT_TOP not in blue.lower() and tint.BACK_TOP not in blue.lower()
    assert tint.is_folder_icon("harbor-folder-documents-glossy")
