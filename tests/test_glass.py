"""Shared glass settings (Harbor Settings > Personalisation, Fusion Desk keys)."""

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "src"))

from harbor import common  # noqa: E402


def test_parse_and_text_round_trip():
    spec = common.parse_glass("effect=SOFT,material=GLOSS,style=GLOW,opacity=60,rim=false")
    assert spec["effect"] == "SOFT" and spec["style"] == "GLOW" and spec["opacity"] == 60
    assert spec["rim"] is False and spec["glare"] is True
    assert common.parse_glass(common.glass_text(spec)) == spec


def test_old_words_and_bad_values():
    assert common.parse_glass("off")["effect"] == "OFF"
    assert common.parse_glass("glossy")["material"] == "GLOSS"
    assert common.parse_glass("liquid")["opacity"] < 100
    spec = common.parse_glass("style=NEON,opacity=5,effect=huge")
    assert spec["style"] == "FROSTED" and spec["opacity"] == 10 and spec["effect"] == "STRONG"


def test_scope_chain_and_quick_choice(tmp_path, monkeypatch):
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path))
    # GLib caches the data folder, so keep the real Fusion Desk out directly.
    monkeypatch.setattr(common.pg, "fusion_settings", lambda: None)
    # Nothing saved: every app follows the desktop (Fusion defaults without Fusion Desk).
    assert common.resolve_glass("files") == common.GLASS_DEFAULT
    assert common.app_glass_choice("files") == "harbor"
    # All Harbor apps follow the desktop: Fusion defaults without Fusion Desk.
    (tmp_path / "harbor").mkdir()
    (tmp_path / "harbor" / "glass").write_text("global")
    assert common.resolve_glass("files") == common.GLASS_DEFAULT
    common.set_app_glass_choice("files", "soft")
    assert common.resolve_glass("files")["effect"] == "SOFT"
    assert common.app_glass_choice("files") == "soft"
    (tmp_path / "harbor" / "glass-files").write_text("style=GLOW\n")
    assert common.app_glass_choice("files") == "custom"
    common.set_app_glass_choice("files", "harbor")
    assert common.app_glass_choice("files") == "harbor"


def test_css_layers():
    assert common.glass_css(common.GLASS_OFF, True) == ""
    css = common.glass_css(common.parse_glass("style=GLOW,opacity=60,shadow=true"), True)
    assert "radial-gradient" in css and "0 4px 14px" in css and "transparent;" in css
    pal = common.glass_palette(common.PALETTES["dark"], common.parse_glass("opacity=60"), True)
    assert pal["card"].startswith("rgba(") and pal["window"].startswith("rgba(")


def test_unusable_file_follows_global(tmp_path, monkeypatch, capsys):
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path))
    monkeypatch.setattr(common.pg, "fusion_settings", lambda: None)
    (tmp_path / "harbor").mkdir()
    (tmp_path / "harbor" / "glass-global").write_text("style=GEL\n")
    (tmp_path / "harbor" / "glass-files").write_bytes(b"\xff\xfe not text")
    (tmp_path / "harbor" / "glass-taskmgr").write_text("nonsense")
    assert common.resolve_glass("files")["style"] == "GEL"     # unreadable: follows, logged
    assert common.resolve_glass("taskmgr")["style"] == "GEL"   # no usable line: follows, logged
    err = capsys.readouterr().err
    assert "cannot read" in err and "no usable line" in err


def test_planner_choice_keeps_lumina_options(tmp_path, monkeypatch):
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path))
    monkeypatch.setattr(common.pg, "fusion_settings", lambda: None)
    (tmp_path / "harbor").mkdir()
    (tmp_path / "harbor" / "glass-global").write_text("lumina=COLOURFUL\nlumina-colours=OCEAN\n")
    common.set_app_glass_choice("files", "planner")
    spec = common.resolve_glass("files")
    assert spec["style"] == "PLANNER" and spec["lumina"] == "COLOURFUL"
    assert spec["lumina-colours"] == "OCEAN"
    assert not list((tmp_path / "harbor").glob("*.tmp"))       # atomic write left no temp file
