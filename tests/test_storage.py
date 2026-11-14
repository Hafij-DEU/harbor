import os

from harbor import files_storage as fs


def test_volume_totals_counts_each_device_once(tmp_path):
    one = fs.volume_totals([str(tmp_path)])
    twice = fs.volume_totals([str(tmp_path), str(tmp_path), os.path.dirname(str(tmp_path))])
    assert one[0] > 0 and one[0] >= one[1] and one[0] >= one[2]
    assert twice[0] >= one[0]
    assert fs.volume_totals([str(tmp_path / "missing")]) == (0, 0, 0)


def test_with_other_fills_rest_and_clamps():
    cats = [("Documents", 30, True), ("Music", 20, False)]
    assert fs.with_other(cats, 100)[-1] == ("Other", 50, True)
    assert fs.with_other(cats, 10)[-1] == ("Other", 0, True)


def test_category_sizes_real_folder(tmp_path):
    d = tmp_path / "Docs"
    d.mkdir()
    (d / "a.bin").write_bytes(b"x" * 5000)
    out = fs.category_sizes([("Documents", str(d)), ("Gone", str(tmp_path / "nope"))])
    assert [o[0] for o in out] == ["Documents"]
    assert out[0][1] >= 5000 and out[0][2]


def test_app_icon_sync(tmp_path, monkeypatch):
    from harbor import files_appicon as ai
    monkeypatch.delenv("PYTEST_CURRENT_TEST", raising=False)
    monkeypatch.setattr(ai, "target", lambda: str(tmp_path / "apps" / (ai.APP_ID + ".svg")))
    path = ai.target()
    assert ai.sync(None) is None
    assert ai.sync("#3584E4") == "written"
    body = open(path).read()
    assert "#3584e4" in body and ai.TOKEN not in body and body.startswith(ai.MARK)
    assert ai.sync("#3584e4") is None            # unchanged: no rewrite
    assert ai.sync(ai._default()) == "removed"
    assert not (tmp_path / "apps" / (ai.APP_ID + ".svg")).exists()
    (tmp_path / "apps" / (ai.APP_ID + ".svg")).write_text("<svg/>")
    assert ai.sync("#ff0000") is None            # not ours: left alone


def test_win12_becomes_custom(tmp_path):
    import json
    from harbor import files_core as fc
    p = tmp_path / "s.json"
    p.write_text(json.dumps({"ui_design": "win12"}))
    known, _extra, _v = fc.Settings.migrate(json.loads(p.read_text()))
    assert known["ui_design"] == "custom"
    assert "custom" in fc.Settings.CHOICES["ui_design"] and "win12" not in fc.Settings.CHOICES["ui_design"]
    assert "merged" in fc.Settings.CHOICES["menu_style"]


def test_merged_menu_has_row_and_no_more_options():
    from harbor import files_menus as fm
    sel = [{"path": "/tmp/a.txt", "name": "a.txt", "is_dir": False, "local": True,
            "content_type": "text/plain", "archive": False}]
    m = fm.merged_item_menu("folder", sel, can_paste=False)
    acts = fm.actions_in(m)
    assert {"cut", "copy", "rename", "delete", "properties"} <= acts
    assert "more-options" not in acts
