"""Tests for Task Manager's Memory Sweep page logic (taskmgr_sweep), the swap
and zram readers of the Performance page, and app folding of the Processes page.

Ported from Memory Sweep's tests. Files are only ever removed under tmp_path
(the RAM folder roots are passed in), processes only ever signalled are python
sleepers the tests start themselves, and the root helper is never run.
"""

import json
import os
import re
import signal
import subprocess
import sys
import time

import pytest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "src"))
os.environ["HARBOR_DRY_RUN"] = "1"

from harbor import taskmgr_proc as tp  # noqa: E402
from harbor import taskmgr_store as ts  # noqa: E402
from harbor import taskmgr_sweep as ms  # noqa: E402


def proc(pid, ppid, exe="", argv=(), comm="", uid=None, ram=1 << 20):
    return ms.Proc(pid, ppid, ms.UID if uid is None else uid, comm or os.path.basename(exe),
                   exe, list(argv), 1000.0 + pid, ram, 0)


# ------------------------------------------------------------------ labels and groups

def test_labels():
    assert ms.label(proc(1, 0, "/usr/bin/python3", ["python3", "-u", "/x/tool.py"]))[1] == \
        "tool (python)"
    assert ms.label(proc(1, 0, "/usr/bin/python3.14", ["python3", "-m", "http.server"]))[0] == \
        "python:http.server"
    assert ms.label(proc(1, 0, "/opt/node", ["node", "/a/.bin/rea", "mcp"]))[1] == "rea (node)"
    assert ms.label(proc(1, 0, "/usr/bin/java", [
        "java", "-cp", "a.jar", "org.gradle.launcher.daemon.bootstrap.GradleDaemon"]))[0] == \
        "gradle-daemon"
    assert ms.label(proc(1, 0, "/usr/bin/java", ["java", "-Xmx1g", "-jar", "/x/app.jar"]))[1] == \
        "app (java)"
    assert ms.label(proc(1, 0, "/snap/brave/1/opt/brave.com/brave/brave", ["brave"]))[1] == "Brave"
    assert ms.label(proc(1, 0, "/home/u/.config/Claude/claude-code/2/x/claude", ["claude"]))[0] \
        == "claude-code"
    assert ms.label(proc(1, 0, "", ["/usr/bin/python3", "/usr/bin/input-remapper-service"],
                         comm="input-remapper-"))[1] == "input-remapper-service (python)"
    assert ms.label(proc(1, 0, "/opt/node", ["npm exec rea-agents@6 mcp"]))[0] == "npm-exec"


def test_groups_and_instances():
    procs = [
        proc(10, 1, "/opt/brave/brave", ["brave"], ram=100),
        proc(11, 10, "/opt/brave/brave", ["brave", "--type=renderer"], ram=50),
        proc(12, 10, "/opt/brave/chrome_crashpad_handler", ["crashpad"], ram=5),
        proc(20, 1, "/opt/node", ["npm exec rea-agents@6 mcp"], ram=7),
        proc(21, 20, "/bin/dash", ["sh", "-c", "rea mcp"], ram=1),
        proc(22, 21, "/opt/node", ["node", "/a/.bin/rea", "mcp"], ram=30),
        proc(30, 1, "/opt/node", ["node", "/a/.bin/rea", "mcp"], ram=40),
    ]
    groups = {g.key: g for g in ms.build_groups(procs)}
    brave = groups["brave"]
    assert len(brave.instances) == 1 and brave.ram == 150
    assert sorted(p.pid for p in brave.instances[0].tree) == [10, 11, 12]
    rea = groups["node:rea"]
    assert len(rea.instances) == 2 and rea.ram == 78  # npm + sh wrappers folded in
    assert "npm-exec" not in groups and "dash" not in groups


def test_protected_and_locks():
    procs = [proc(5, 1, "/usr/bin/gnome-shell", ["gnome-shell"]),
             proc(6, 1, "/usr/sbin/snapd", ["snapd"], uid=0),
             proc(7, 1, "/usr/bin/foo", ["foo"]),
             proc(8, 1, "/usr/bin/bar", ["bar"]),
             proc(9, 1, "/usr/lib/systemd/systemd", ["/usr/lib/systemd/systemd", "--user"])]
    groups = {g.key: g for g in ms.build_groups(procs, keep=frozenset({8}))}
    assert groups["gnome-shell"].locked
    assert ms.is_protected(procs[0]) and ms.is_protected(procs[4])
    assert not ms.is_protected(procs[2])
    assert groups["snapd"].locked == "Runs as another user"
    assert groups["foo"].locked is None
    assert groups["bar"].locked == "Task Manager was started from this app"
    me = proc(os.getpid(), 1, "/usr/bin/python3", ["python3", "harbor-taskmgr"])
    [group] = ms.build_groups([me], keep=frozenset({os.getpid()}))
    assert group.locked == "This is Task Manager itself"


def test_real_scan_finds_self():
    procs = ms.scan_procs()
    me = next(p for p in procs if p.pid == os.getpid())
    assert me.own and me.ram > 0 and abs(me.start - time.time()) < 3600 * 24 * 30
    assert me.ticks is not None
    assert os.getpid() in ms.self_and_ancestors()


def test_procs_from_sampler_reuses_identity():
    s = tp.ProcSampler()
    s.sample()
    btime = ms.boot_time()
    procs = {p.pid: p for p in ms.procs_from_sampler(s.procs, btime)}
    me = procs[os.getpid()]
    assert me.own and me.ram > 0 and me.ticks == s.procs[os.getpid()].start
    assert abs(me.start - next(p.start for p in ms.scan_procs() if p.pid == os.getpid())) < 1
    assert not any(p.kernel for p in s.procs.values() if p.pid in procs)


# ------------------------------------------------------------------ extra copies

MESH = "/opt/MeshDrop/meshdrop-desktop"


def meshdrop_tree():
    """MeshDrop as seen on 2026-10-31: the main process plus a second full copy
    (the clipboard helper) with its own zygote, GPU and network processes."""
    def m(pid, ppid, args="", ram=1 << 20):
        return proc(pid, ppid, MESH, [f"{MESH} {args}".strip()], comm="meshdrop-deskto", ram=ram)
    return [
        proc(50, 1, "/usr/lib/systemd/systemd", ["/usr/lib/systemd/systemd", "--user"]),
        m(100, 50, ram=68 << 20),
        m(101, 100, "--type=zygote --no-zygote-sandbox"),
        m(102, 101, "--type=gpu-process --ozone-platform=wayland", ram=35 << 20),
        m(103, 100, "--type=zygote"),
        m(104, 103, "--type=renderer", ram=53 << 20),
        m(105, 100, "--type=utility --utility-sub-type=network.mojom.NetworkService"),
        proc(106, 100, "/usr/bin/python3.14", ["python3", "/opt/MeshDrop/resources/meshdrop-bt.py"]),
        m(110, 100, "--password-store=basic", ram=28 << 20),
        m(111, 110, "--type=zygote --no-zygote-sandbox"),
        m(112, 111, "--type=gpu-process", ram=10 << 20),
        m(113, 112, "--type=broker"),
        m(114, 110, "--type=zygote"),
        m(115, 110, "--type=utility --utility-sub-type=network.mojom.NetworkService", ram=7 << 20),
    ]


def test_extra_copy_found_for_meshdrop():
    groups = {g.key: g for g in ms.build_groups(meshdrop_tree())}
    mesh = groups["meshdrop-desktop"]
    assert mesh.title == "MeshDrop"
    [extra] = mesh.extras
    assert extra.root.pid == 110  # the clipboard helper, never the main process 100
    assert sorted(p.pid for p in extra.tree) == [110, 111, 112, 113, 114, 115]
    assert extra.ram == (28 + 1 + 10 + 1 + 1 + 7) << 20
    assert "clipboard" in mesh.extra_note
    assert groups["systemd"].extras == []


def test_extra_copy_not_found_elsewhere():
    brave = [proc(10, 1, "/opt/brave/brave", ["/opt/brave/brave"]),
             proc(11, 10, "/opt/brave/brave", ["/opt/brave/brave --type=renderer"]),
             proc(12, 10, "/opt/brave/brave", ["/opt/brave/brave --type=gpu-process"])]
    shells = [proc(20, 1, "/bin/bash", ["bash"]), proc(21, 20, "/bin/bash", ["bash"]),
              proc(22, 21, "/bin/bash", ["bash"])]
    pool = [proc(30, 1, "/usr/bin/python3", ["python3", "/x/job.py"]),
            proc(31, 30, "/usr/bin/python3", ["python3", "/x/job.py"])]
    odd = [proc(40, 1, "/opt/x/x", ["/opt/x/x"]),
           proc(41, 40, "/opt/x/x", ["/opt/x/x --type=utility"]),
           proc(42, 41, "/opt/x/x", ["/opt/x/x"]),
           proc(43, 42, "/opt/x/x", ["/opt/x/x --type=gpu-process"])]
    for g in ms.build_groups(brave + shells + pool + odd):
        assert g.extras == [], g.key


def test_extra_copy_inside_extra_copy_counted_once():
    tree = meshdrop_tree() + [
        proc(120, 110, MESH, [f"{MESH} --nested"]),
        proc(121, 120, MESH, [f"{MESH} --type=zygote"])]
    [mesh] = [g for g in ms.build_groups(tree) if g.key == "meshdrop-desktop"]
    [extra] = mesh.extras
    assert extra.root.pid == 110 and {120, 121} <= {p.pid for p in extra.tree}


def test_extra_copy_locks():
    other = [proc(p.pid, p.ppid, p.exe, p.argv, uid=ms.UID + 1) for p in meshdrop_tree()]
    [mesh] = [g for g in ms.build_groups(other) if g.key == "meshdrop-desktop"]
    assert ms.instance_locked(mesh.extras[0]) == "Runs as another user"
    [mesh] = [g for g in ms.build_groups(meshdrop_tree()) if g.key == "meshdrop-desktop"]
    assert ms.instance_locked(mesh.extras[0], keep=frozenset({113})) == \
        "Task Manager was started from this app"
    assert ms.instance_locked(mesh.extras[0]) is None


def test_extra_copy_tip():
    snap = ms.Snapshot(procs=meshdrop_tree())
    [tip] = [t for t in snap.tips() if t[0] == "extra"]
    assert tip[2:] == ("apps", "meshdrop-desktop") and "MeshDrop" in tip[1] and "48 MB" in tip[1]
    assert snap.group("meshdrop-desktop").title == "MeshDrop"
    small = meshdrop_tree()
    for p in small:
        if p.pid >= 110:
            p.ram = 1 << 20
    assert not [t for t in ms.Snapshot(procs=small).tips() if t[0] == "extra"]


# ------------------------------------------------------------------ ending processes

@pytest.fixture
def sleeper_tree():
    """A python parent with a python child, both sleeping (never /bin/sleep:
    uutils is a multicall binary)."""
    code = ("import subprocess, sys, time; "
            "subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)']); "
            "time.sleep(60)")
    p = subprocess.Popen([sys.executable, "-c", code])
    deadline = time.time() + 5
    inst = None
    while time.time() < deadline:
        inst = next((i for g in ms.build_groups(ms.scan_procs()) for i in g.instances
                     if i.root.pid == p.pid), None)
        if inst is not None and len(inst.tree) == 2:
            break
        time.sleep(0.1)
    yield p, inst
    for q in (inst.tree if inst else []):
        if ms.alive(q.pid, q.start):
            os.kill(q.pid, signal.SIGKILL)
    if p.poll() is None:
        p.kill()
    p.wait(timeout=5)


def test_stop_dry_run_only_logs(sleeper_tree):
    p, inst = sleeper_tree
    before = len(ms.ACTION_LOG)
    assert ms.stop([inst], dry_run=True) == []
    assert p.poll() is None
    assert len(ms.ACTION_LOG) > before


def test_stop_ends_tree(sleeper_tree):
    p, inst = sleeper_tree
    child = [q for q in inst.tree if q.pid != p.pid]
    assert child
    assert ms.stop([inst], wait=3, dry_run=False) == []
    assert p.wait(timeout=5) is not None
    for c in child:
        assert not ms.alive(c.pid, c.start)


def test_stop_respects_keep_and_pid_reuse(sleeper_tree):
    p, inst = sleeper_tree
    ms.stop([inst], keep=frozenset(q.pid for q in inst.tree), wait=0.5, dry_run=False)
    assert p.poll() is None  # kept pids are never signalled
    # a stale start time must not be signalled (pid reuse guard)
    assert not ms._send(p.pid, inst.root.start - 100, signal.SIGKILL, dry_run=False)
    # nor a pidfd check with the wrong start ticks
    assert not ms._send(p.pid, inst.root.start, signal.SIGKILL, ticks=inst.root.ticks + 5,
                        dry_run=False)
    time.sleep(0.2)
    assert p.poll() is None


# ------------------------------------------------------------------ files in memory

@pytest.fixture
def ram_root(tmp_path):
    """A stand-in RAM folder: roots are passed to the functions explicitly."""
    root = tmp_path / "shm"
    (root / "dir" / "sub").mkdir(parents=True)
    (root / "dir" / "sub" / "f").write_bytes(b"x" * 100000)
    (root / "small").write_bytes(b"y" * 10)
    return root


def test_measure_list_and_remove(ram_root):
    d = str(ram_root / "dir")
    size, mtime, sockets = ms.measure(d)
    assert size >= 100000 and sockets == 0 and mtime > 0
    items = ms.list_items(roots=[str(ram_root)], markers=(set(), set()))
    names = [i.name for i in items]
    assert names[0] == "dir" and "small" in names  # biggest first
    it = items[0]
    assert it.own and it.is_dir and it.unused
    assert ms.removable(d, roots=[str(ram_root)]) is None
    # dry run keeps the files and logs the command
    assert ms.remove(d, roots=[str(ram_root)], dry_run=True) == 0
    assert os.path.exists(d) and ms.ACTION_LOG[-1] == ["rm", "-rf", d]
    assert ms.remove(d, roots=[str(ram_root)], dry_run=False) == 0
    assert not os.path.exists(d)


def test_in_use_marker(ram_root):
    d = str(ram_root / "dir")
    [it] = [i for i in ms.list_items(roots=[str(ram_root)], markers=({d}, set()))
            if i.name == "dir"]
    assert it.in_use and not it.unused


def test_remove_refuses_outside_and_symlink_escape(tmp_path, ram_root):
    roots = [str(ram_root)]
    outside = tmp_path / "keep.txt"
    outside.write_text("important")
    assert ms.removable(str(outside), roots=roots) is not None
    with pytest.raises(PermissionError):
        ms.remove(str(outside), roots=roots, dry_run=False)
    with pytest.raises(PermissionError):
        ms.remove(str(ram_root), roots=roots, dry_run=False)  # the root itself
    with pytest.raises(PermissionError):
        ms.remove(str(ram_root) + "/", roots=roots, dry_run=False)  # empty basename
    with pytest.raises(PermissionError):
        ms.remove("/etc/hostname", roots=roots, dry_run=False)
    # a symlink inside the RAM folder pointing outside: removing it drops only the link
    link = ram_root / "link"
    os.symlink(str(outside), link)
    assert ms.remove(str(link), roots=roots, dry_run=False) == 0
    assert outside.read_text() == "important" and not os.path.lexists(link)
    # a symlinked parent folder must not smuggle in an outside path
    os.symlink(str(tmp_path), ram_root / "dirlink")
    with pytest.raises(PermissionError):
        ms.remove(str(ram_root / "dirlink" / "keep.txt"), roots=roots, dry_run=False)
    assert outside.exists()
    # a folder name that only starts like the root is not inside it
    twin = tmp_path / "shm-twin"
    twin.mkdir()
    assert ms.removable(str(twin / "x"), roots=roots) is not None


def test_remove_unused_skips_busy(ram_root):
    roots = [str(ram_root)]
    (ram_root / "busy").mkdir()
    items = ms.list_items(roots=roots, markers=({str(ram_root / "busy")}, set()))
    other = ms.Item(str(ram_root / "small"), 10)
    other.own = False
    n, freed, skipped = ms.remove_unused(items + [other], roots=roots, dry_run=False)
    assert n == 2 and freed >= 100000
    assert sorted(r for _i, r in skipped) == ["in use", "not yours"]
    assert (ram_root / "busy").exists()
    assert not (ram_root / "dir").exists() and not (ram_root / "small").exists()


def test_describe():
    uid = ms.UID
    assert ms.describe(f"/tmp/claude-{uid}") == "Claude Code temp files"
    assert ms.describe(f"/tmp/claude-{uid}/-home-u-apps/dfca909e-4619-483d-aecd-be8cbfb8bcfe") \
        == "Claude Code session temp files"
    assert ms.describe("/tmp/node-compile-cache").startswith("Compile cache")


def test_hidden_usage():
    it = ms.Item("/dev/shm/a", 10 << 20)
    assert ms.hidden_usage([it], [("/dev/shm", 100 << 20, 1 << 30)]) == {"/dev/shm": 90 << 20}
    assert ms.hidden_usage([it], [("/dev/shm", 11 << 20, 1 << 30)]) == {}


def test_report_json(capsys):
    assert ms.report(as_json=True) == 0
    data = json.loads(capsys.readouterr().out)
    assert data["ram"]["total"] > 0 and data["apps"]


def test_cli_sweep_report():
    out = subprocess.run([sys.executable, "-m", "harbor.taskmgr", "sweep-report", "--json"],
                         capture_output=True, text=True, check=True, timeout=60,
                         cwd=os.path.join(ROOT, "src"),
                         env=dict(os.environ, PYTHONDONTWRITEBYTECODE="1")).stdout
    assert json.loads(out)["ram"]["total"] > 0


# ------------------------------------------------------------------ memory optimization

def swap_state(**kw):
    st = {"ram": 16 << 30, "tools": True, "zram_on": True, "zram_percent": 50, "algo": "zstd",
          "zram_size": 8 << 30, "zram_used": 0, "zram_stored": 0, "zram_ram": 0,
          "file": "/swap.img", "file_size": 4 << 30, "file_used": 0, "file_active": True,
          "disk_free": 40 << 30, "zswap": False, "zswap_compressor": "zstd",
          "swappiness": 60, "cache_pressure": 100}
    st.update(kw)
    return st


def test_swap_form_matches_system():
    st = swap_state()
    form = ms.swap_form(st)
    assert form == {"tools": None, "zram": 50, "algo": "zstd", "file": 4, "zswap": False,
                    "swappiness": 60, "cache_pressure": 100}
    assert ms.plan_changes(st, form) == []
    off = ms.swap_form(swap_state(zram_on=False, file_size=0))
    assert off["zram"] == 0 and off["file"] == 0


def test_plan_changes():
    st = swap_state()
    form = dict(ms.swap_form(st), zram=100, file=8, zswap=True, swappiness=150)
    assert ms.plan_changes(st, form) == ["zram=100:zstd", "file=8", "zswap=1", "swappiness=150"]
    assert ms.plan_changes(st, dict(ms.swap_form(st), algo="lz4")) == ["zram=50:lz4"]
    nz = swap_state(zswap=None)
    assert ms.plan_changes(nz, dict(ms.swap_form(nz), zswap=True)) == []
    assert ms.plan_changes(st, dict(ms.swap_form(st), file=0)) == []


def test_plan_changes_tools():
    bare = swap_state(tools=False, zram_on=False, zram_percent=0)
    form = ms.swap_form(bare)
    assert ms.plan_changes(bare, dict(form, zram=50)) == []
    assert ms.plan_changes(bare, dict(form, tools="install", zram=50)) == \
        ["tools=install", "zram=50:zstd"]
    st = swap_state()
    assert ms.plan_changes(st, dict(ms.swap_form(st), tools="remove", zram=80)) == \
        ["tools=remove"]


def test_describe_change():
    assert ms.describe_change("zram=0:lz4") == "Turn zram off"
    assert ms.describe_change("zram=75:zstd") == "zram at 75% of RAM, packed with zstd"
    assert ms.describe_change("file=6") == "Swap file of 6 GB"
    assert ms.describe_change("zswap=1") == "Turn zswap on"
    assert ms.describe_change("tools=remove") == "Remove zram-tools and turn zram off"
    assert ms.describe_change("cache_pressure=50") == "Cache pressure 50"


def test_limits_and_defaults_match_memory_sweep():
    assert ms.LIMITS == {"zram": (0, 150, 5), "file": (1, 64, 1), "swappiness": (0, 200, 5),
                         "cache_pressure": (1, 500, 10)}
    assert ms.SWAP_HELPER == "/usr/libexec/harbor/harbor-swap"
    assert ms.POLKIT_ACTION == "io.github.hafij_deu.Harbor.swap"


def test_swap_command_building():
    cmd = ms.swap_command(["zram=50:zstd", "file=4"])
    assert cmd[0] == "pkexec" and cmd[-2:] == ["zram=50:zstd", "file=4"]
    if os.access(ms.SWAP_HELPER, os.X_OK):
        assert cmd[1] == ms.SWAP_HELPER
    else:
        assert cmd[1:3] == ["/bin/sh", os.path.join(ROOT, "data", "harbor-swap")]
    assert os.path.isfile(ms.source_helper())


def test_swap_result():
    assert ms.swap_result(0) == "Changes applied"
    assert ms.swap_result(126) == ms.swap_result(127) == "Cancelled"
    msg = ms.swap_result(4, "ok zram\nok file\n")
    assert msg.startswith(ms.SWAP_ERRORS[4]) and msg.endswith("Done before that: zram, file")
    assert "code 99" in ms.swap_result(99)


def test_swap_status_reads_system():
    st = ms.swap_status()
    assert st["ram"] > 0
    assert st["algo"] in ms.ALGOS
    assert st["zswap"] in (None, True, False)
    assert 0 <= st["swappiness"] <= 200


def test_helper_syntax_and_policy():
    helper = os.path.join(ROOT, "data", "harbor-swap")
    assert subprocess.run(["sh", "-n", helper]).returncode == 0
    src = open(helper).read()
    # its own config files: never the ones the memory-sweep package owns
    assert "91-harbor-memory.conf" in src and "harbor-zswap.conf" in src
    assert "SYSCTL=/etc/sysctl.d/90-memory-sweep.conf" not in src
    from xml.dom import minidom
    doc = minidom.parse(os.path.join(ROOT, "data", "io.github.hafij_deu.Harbor.policy"))
    [action] = doc.getElementsByTagName("action")
    assert action.getAttribute("id") == ms.POLKIT_ACTION
    annos = {a.getAttribute("key"): a.firstChild.data
             for a in doc.getElementsByTagName("annotate")}
    assert annos["org.freedesktop.policykit.exec.path"] == ms.SWAP_HELPER


# ------------------------------------------------------------------ high-memory warning

def test_should_notify_rate_limit():
    assert ms.should_notify(95, True, 90, last=-1e9, now=0)
    assert not ms.should_notify(95, False, 90, last=-1e9, now=0)
    assert not ms.should_notify(80, True, 90, last=-1e9, now=0)
    assert not ms.should_notify(95, True, 90, last=0, now=ms.NOTIFY_EVERY - 1)
    assert ms.should_notify(95, True, 90, last=0, now=ms.NOTIFY_EVERY)


def test_notification_text():
    [g] = [g for g in ms.build_groups(meshdrop_tree()) if g.key == "meshdrop-desktop"]
    title, body = ms.notification_text(93.4, [g])
    assert title == "RAM is almost full" and body.startswith("93% of RAM") and "MeshDrop" in body
    assert ms.notification_text(91, [])[1] == "91% of RAM is in use."


def test_settings_defaults(tmp_path):
    s = ts.Settings(str(tmp_path / "taskmgr.json"))
    assert s["notify_memory"] is True and s["notify_at"] == 90
    s["notify_at"] = 85
    assert ts.Settings(str(tmp_path / "taskmgr.json"))["notify_at"] == 85


def test_notification_methods_exist():
    # The warning is sent from a GLib idle callback, where a bad method name
    # would only surface as a crash report.
    from gi.repository import Gio
    src = open(os.path.join(ROOT, "src", "harbor", "taskmgr.py")).read()
    body = src[src.index("def _check_memory("):]
    body = body[:body.index("\n    def ", 1)]
    calls = set(re.findall(r"\bn\.(\w+)\(", body))
    assert "set_default_action_and_target" in calls
    assert all(hasattr(Gio.Notification, c) for c in calls), calls


def test_nav_order_and_shortcuts():
    pytest.importorskip("gi")
    from harbor.taskmgr import NAV, PAGE_KEYS
    keys = [k for k, _l, _i in NAV]
    assert keys[:3] == ["processes", "performance", "sweep"]
    assert dict((k, l) for k, l, _i in NAV)["sweep"] == "Memory Sweep"
    assert "sweep" in PAGE_KEYS and "settings" in PAGE_KEYS


# ------------------------------------------------------------------ swap / vmstat / zram parsers

SWAPS = """Filename\t\t\t\tType\t\tSize\t\tUsed\t\tPriority
/swap.img                               file\t\t4194300\t\t1048576\t\t-2
/dev/zram0                              partition\t16777212\t\t4096\t\t100
/mnt/my\\040disk/swap                   file\t\t1024\t\t0\t\t-3
"""


def test_parse_swaps():
    rows = tp.parse_swaps(SWAPS)
    assert [r["name"] for r in rows] == ["/swap.img", "/dev/zram0", "/mnt/my disk/swap"]
    f, z, s = rows
    assert (f["kind"], f["size"], f["used"], f["priority"]) == \
        ("file", 4194300 * 1024, 1 << 30, -2)
    assert z["type"] == "partition" and z["kind"] == "zram" and z["priority"] == 100
    assert tp.swap_kind_name(z["kind"]).startswith("Compressed RAM")
    assert tp.swap_kind_name("partition") == "Partition"
    assert tp.parse_swaps("") == [] and tp.parse_swaps(SWAPS.splitlines()[0]) == []
    assert tp.parse_swaps("h\n/x file nope 0 0\n") == []


def test_parse_vmstat_and_rates():
    a = tp.parse_vmstat("nr_free_pages 100\npswpin 10\npswpout 20\nbogus x\n")
    b = tp.parse_vmstat("pswpin 14\npswpout 20\n")
    assert a == {"nr_free_pages": 100, "pswpin": 10, "pswpout": 20}
    assert tp.swap_rates(a, b, 2.0, page_size=4096) == (4 * 4096 / 2.0, 0.0)
    assert tp.swap_rates(None, b, 1.0) == (0.0, 0.0)
    assert tp.swap_rates(a, b, 0) == (0.0, 0.0)
    # counters never go backwards; a reset reads as no traffic
    assert tp.swap_rates(b, a, 1.0, page_size=4096) == (0.0, 0.0)


def test_parse_mm_stat_and_algorithm():
    mm = tp.parse_mm_stat("4898123776 1474351104 1519316992 0 1600000000 12345 0 7 9\n")
    assert mm["orig"] == 4898123776 and mm["compr"] == 1474351104
    assert mm["mem_used"] == 1519316992 and mm["same_pages"] == 12345 and mm["huge_pages"] == 7
    assert tp.parse_mm_stat("") == {}
    assert tp.parse_mm_stat("1 2 3") == {"orig": 1, "compr": 2, "mem_used": 3}
    assert tp.parse_comp_algorithm("lzo lzo-rle lz4 lz4hc 842 [zstd]\n") == "zstd"
    assert tp.parse_comp_algorithm("lz4") == "lz4"
    assert tp.parse_comp_algorithm("") == ""


def fake_sys(tmp_path, zram_size=16 << 30):
    sysr = tmp_path / "sys"
    z = sysr / "block" / "zram0"
    z.mkdir(parents=True)
    (z / "disksize").write_text("%d\n" % zram_size)
    (z / "mm_stat").write_text("4000 1000 1200 0 1300 0 0 0 0\n")
    (z / "comp_algorithm").write_text("lzo [lz4] zstd\n")
    (z / "stat").write_text(" ".join(["0"] * 17) + "\n")
    unused = sysr / "block" / "zram1"
    unused.mkdir()
    (unused / "disksize").write_text("0\n")
    sda = sysr / "block" / "sda"
    (sda / "queue").mkdir(parents=True)
    (sda / "size").write_text("1000000\n")
    (sda / "stat").write_text(" ".join(["0"] * 17) + "\n")
    (sda / "queue" / "rotational").write_text("0\n")
    return sysr


def test_zram_devices(tmp_path):
    sysr = fake_sys(tmp_path)
    [d] = tp.zram_devices(str(sysr))
    assert d == {"name": "zram0", "disksize": 16 << 30, "algo": "lz4", "orig": 4000,
                 "compr": 1000, "mem_used": 1200, "ratio": 4.0}
    assert tp.zram_devices(str(tmp_path / "nothing")) == []


def test_sampler_swap_fields_and_no_zram_disk(tmp_path):
    from test_taskmgr import Clock, proc_stat_text
    sysr = fake_sys(tmp_path)
    pr = tmp_path / "proc"
    pr.mkdir()
    (pr / "stat").write_text(proc_stat_text(2))
    (pr / "meminfo").write_text("MemTotal: 16000000 kB\nMemFree: 1000000 kB\n"
                                "MemAvailable: 8000000 kB\nSwapTotal: 20971516 kB\n"
                                "SwapFree: 19922940 kB\n")
    (pr / "swaps").write_text(SWAPS)
    (pr / "vmstat").write_text("pswpin 100\npswpout 100\n")
    clock = Clock()
    s = tp.SystemSampler(proc_root=str(pr), sys_root=str(sysr), clock=clock)
    # zram is compressed RAM, not a disk
    assert [d["name"] for d in s.disks] == ["sda"]
    s.sample({}, 1000)
    assert [d["name"] for d in s.swaps] == ["/swap.img", "/dev/zram0", "/mnt/my disk/swap"]
    total = sum(d["size"] for d in s.swaps)
    used = sum(d["used"] for d in s.swaps)
    assert s.swap_hist[-1] == pytest.approx(100 * used / total)
    assert s.swap_now == (0.0, 0.0)
    assert [z["name"] for z in s.zram] == ["zram0"]
    (pr / "vmstat").write_text("pswpin 110\npswpout 100\n")
    clock.t += 2.0
    s.sample({}, 1000)
    assert s.swap_now == (10 * tp.PAGE_SIZE / 2.0, 0.0)
    assert s.swap_io_hist[0][-1] == s.swap_now[0]


# ------------------------------------------------------------------ app folding (Processes)

MESH_MAIN_CG = ("/user.slice/user-1000.slice/user@1000.service/app.slice/"
                "app-meshdrop-7298.scope")
MESH_HELPER_CG = ("/user.slice/user-1000.slice/user@1000.service/app.slice/"
                  "app-gnome-io.github.hafijbge.meshdrop-7298.scope")
DISK_NOTIFY_CG = ("/user.slice/user-1000.slice/user@1000.service/app.slice/"
                  "app-gnome-org.gnome.DiskUtility.Notify-1234.scope")
UPDATE_CG = ("/user.slice/user-1000.slice/user@1000.service/app.slice/"
             "app-gnome-update\\x2dnotifier-99.scope")


def test_meshdrop_helpers_fold_into_one_app(tmp_path):
    pytest.importorskip("gi")
    from test_taskmgr import Clock, make_proc, proc_stat_text
    from harbor.taskmgr_model import AppLookup, ProcModels
    root = tmp_path / "proc"
    root.mkdir()
    (root / "stat").write_text(proc_stat_text(2))
    make_proc(root, 100, "meshdrop-deskto", cgroup=MESH_MAIN_CG, argv=[MESH])
    make_proc(root, 101, "meshdrop-deskto", cgroup=MESH_MAIN_CG, ppid=100,
              argv=[MESH, "--type=zygote"])
    make_proc(root, 110, "meshdrop-deskto", cgroup=MESH_HELPER_CG, argv=[MESH, "--clipboard"])
    make_proc(root, 111, "meshdrop-deskto", cgroup=MESH_HELPER_CG, ppid=110,
              argv=[MESH, "--type=gpu-process"])
    make_proc(root, 200, "gsd-disk-utilit", cgroup=DISK_NOTIFY_CG,
              argv=["/usr/libexec/gsd-disk-utility-notify"])
    make_proc(root, 300, "update-notifier", cgroup=UPDATE_CG, argv=["update-notifier"])
    entries = [
        {"id": "meshdrop", "name": "MeshDrop", "icon": None, "visible": True,
         "wmclass": "meshdrop-desktop", "exec": MESH},
        # the helper's .desktop exists only in autostart, so it is not listed
        {"id": "org.gnome.DiskUtility.Notify", "name": "Disk Notify", "icon": None,
         "visible": False, "wmclass": "", "exec": "/usr/libexec/gsd-disk-utility-notify"},
        {"id": "update-notifier", "name": "Software Updater", "icon": None, "visible": False,
         "wmclass": "", "exec": "update-notifier"},
        {"id": "org.gnome.Nautilus", "name": "Files", "icon": None, "visible": True,
         "wmclass": "org.gnome.Nautilus", "exec": "nautilus"},
    ]
    lookup = AppLookup(entries=entries)
    assert lookup.resolve({"id": "io.github.hafijbge.meshdrop", "kind": "scope",
                           "desktop": ["io.github.hafijbge.meshdrop.desktop"]})[:2] == \
        ("meshdrop", "MeshDrop")
    assert lookup.resolve({"id": "update-notifier", "kind": "scope",
                           "desktop": ["update-notifier.desktop"]}) is None
    s = tp.ProcSampler(root=str(root), ncpu=2, my_uid=1000, clock=Clock(), hz=100)
    m = ProcModels(lookup, my_uid=1000)
    m.update(s.sample(), 16 * 1024 ** 3)
    apps = m.groups["apps"].children
    labels = [apps.get_item(i).label for i in range(apps.get_n_items())]
    assert labels == ["MeshDrop (4)"]
    bg = m.groups["background"].children
    bg_pids = set()
    for i in range(bg.get_n_items()):
        it = bg.get_item(i)
        bg_pids |= {p.pid for p in getattr(it, "procs", [])} or {getattr(it, "pid", None)}
    assert {200, 300} <= bg_pids
