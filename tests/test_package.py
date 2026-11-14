"""Package preview (.deb / .apk) and drop-to-pin in the navigation pane."""
import glob
import shutil
import subprocess

import pytest

from harbor import files_core as fc
from harbor import files_package as fp
from harbor import files_sidebar as fsb

APKS = glob.glob("/mnt/E6CEDEF1CEDEB8CF/all app/Android APKs/*.apk")


def test_preview_kind_package():
    assert fc.preview_kind("application/vnd.debian.binary-package", "a.deb") == "package"
    assert fc.preview_kind("", "App.APK") == "package"


def test_parse_control_folds_long_description():
    info = fp.parse_control("Package: hex\nVersion: 1.2\nDescription: short\n line two\n .\n end\n")
    assert info["Package"] == "hex" and info["Version"] == "1.2"
    assert info["Description"].startswith("short\n")
    assert "line two" in info["Description"]


@pytest.mark.skipif(shutil.which("dpkg") is None, reason="needs dpkg")
def test_install_state():
    assert fp.install_state("1.0", None) == "not-installed"
    assert fp.install_state("1.1", "1.0") == "update"
    assert fp.install_state("1.0", "1.0") == "installed"
    assert fp.install_state("1.0", "1.0.1") == "newer-installed"


def test_icon_rank_skips_symbolic():
    assert fp._icon_rank("./usr/share/icons/hicolor/scalable/apps/x-symbolic.svg") is None
    assert fp._icon_rank("./usr/share/icons/hicolor/scalable/apps/x.svg")
    assert fp._icon_rank("./usr/share/doc/x/README") is None


@pytest.mark.skipif(shutil.which("dpkg-deb") is None, reason="needs dpkg-deb")
def test_deb_info_and_icon(tmp_path):
    root = tmp_path / "pkg"
    (root / "DEBIAN").mkdir(parents=True)
    (root / "DEBIAN" / "control").write_text(
        "Package: harbor-test-pkg\nVersion: 0.1\nArchitecture: all\n"
        "Maintainer: T <t@example.com>\nInstalled-Size: 4\nDescription: test package\n")
    icon = root / "usr/share/icons/hicolor/scalable/apps"
    icon.mkdir(parents=True)
    (icon / "harbor-test-pkg.svg").write_text("<svg xmlns='http://www.w3.org/2000/svg'/>")
    deb = tmp_path / "t.deb"
    subprocess.run(["dpkg-deb", "--root-owner-group", "--build", str(root), str(deb)],
                   check=True, capture_output=True)
    info = fp.deb_info(str(deb))
    assert info["Package"] == "harbor-test-pkg" and info["state"] == "not-installed"
    data, ext = fp.deb_icon(str(deb))
    assert ext == ".svg" and data.startswith(b"<svg")


def test_deb_info_rejects_junk(tmp_path):
    junk = tmp_path / "x.deb"
    junk.write_bytes(b"not a deb")
    assert not fp.deb_info(str(junk))
    assert fp.apk_info(str(junk)) is None


@pytest.mark.skipif(not APKS, reason="no APKs to read")
def test_apk_info_real_file():
    info = fp.apk_info(APKS[0])
    assert info and "." in info["package"] and isinstance(info["permissions"], list)
    assert fp.android_name(26).startswith("Android")


def test_pin_dropped(tmp_path):
    s = fc.Settings(str(tmp_path / "files.json"))
    d = tmp_path / "work"
    d.mkdir()
    f = tmp_path / "file.txt"
    f.write_text("x")
    assert fsb.pin_dropped(s, [str(d), str(f), None]) == [str(d)]
    assert fsb.is_pinned(s, str(d))
    assert fsb.pin_dropped(s, [str(d)]) == []


def test_files_css_uses_only_palette_names():
    """An unknown $name makes Template.substitute raise and drops the whole theme."""
    from string import Template
    from harbor import common, files
    for pal in common.PALETTES.values():
        Template(files.CSS).substitute(pal)
