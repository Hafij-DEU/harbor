from harbor import files_core as fc

OUT = """live wlp0s20f3 off
conn aaaa-1 disable
conn bbbb-2 default
"""


def test_state_parses_live_and_profiles():
    st = fc.wifi_ps_state(OUT)
    assert st == {"live": {"wlp0s20f3": False}, "conns": {"aaaa-1": "disable", "bbbb-2": "default"},
                  "global": ""}


def test_saved_on_counts_default_as_on():
    assert fc.wifi_ps_saved_on(fc.wifi_ps_state(OUT)) is True
    assert fc.wifi_ps_saved_on(fc.wifi_ps_state("conn a disable\nconn b disable\n")) is False


def test_default_profile_follows_global_setting():
    assert fc.wifi_ps_saved_on(fc.wifi_ps_state(OUT + "global 2\n")) is False
    assert fc.wifi_ps_saved_on(fc.wifi_ps_state(OUT + "global 3\n")) is True
    assert fc.wifi_ps_saved_on(fc.wifi_ps_state("conn a enable\nglobal 2\n")) is True


def test_saved_on_none_without_wifi():
    assert fc.wifi_ps_saved_on(fc.wifi_ps_state("")) is None
    assert fc.wifi_ps_saved_on(fc.wifi_ps_state("live wlan0\n")) is None


def test_set_argv_passes_uuids_as_arguments():
    argv = fc.wifi_ps_set_argv(["u1", "u;rm -rf /"], False)
    assert argv[:2] == ["sh", "-c"] and "powersave 2" in argv[2]
    assert argv[3:] == ["sh", "u1", "u;rm -rf /"]
    assert "powersave 3" in fc.wifi_ps_set_argv(["u1"], True)[2]


def test_query_reads_without_changing():
    argv = fc.wifi_ps_query_argv()
    assert argv[0] == "sh" and "modify" not in argv[2]


def test_share_asks_only_while_power_saving_on(monkeypatch):
    from harbor import files_wifi_ps as w
    shown, ran = [], []

    def fake_run(argv, cb, mutating=True):
        cb(True, OUT_ON, "")
    monkeypatch.setattr(w.common, "run_cmd", fake_run)
    monkeypatch.setattr(w, "_show", lambda win, conns, then: shown.append(conns))
    OUT_ON = "conn a default\nglobal 3\n"
    w.ask_if_on(None, lambda: ran.append(1))
    assert shown == [["a"]] and ran == []
    OUT_ON = "conn a default\nglobal 2\n"
    w.ask_if_on(None, lambda: ran.append(1))
    assert ran == [1] and len(shown) == 1


def test_warning_explains_effects():
    from harbor import files_wifi_ps as w
    for words in ("slow", "Camera", "time out", "battery", "Settings"):
        assert words in w.BODY
