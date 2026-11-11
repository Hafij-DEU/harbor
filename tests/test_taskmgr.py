"""Tests for Harbor Task Manager: parsers, grouping, guards and stores.

Everything runs against a fake /proc tree under tmp_path; the only real
process touched is a python sleeper the tests start themselves.
"""

import json
import os
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


# ------------------------------------------------------------------ fake /proc

def stat_line(pid, comm, state="S", ppid=1, utime=0, stime=0, nice=0, threads=1,
              start=1000, rss=100, flags=0):
    f = [state, ppid, pid, pid, 0, -1, flags, 0, 0, 0, 0, utime, stime, 0, 0, 20, nice,
         threads, 0, start, 1000000, rss]
    f += [0] * 30
    return "%d (%s) %s\n" % (pid, comm, " ".join(str(x) for x in f))


def status_text(uid, anon_kb=1000, shmem_kb=24, rss_kb=5000):
    return ("Name:\tx\nUmask:\t0022\nState:\tS (sleeping)\nPid:\t1\nUid:\t%d\t%d\t%d\t%d\n"
            "Gid:\t1000\t1000\t1000\t1000\nVmRSS:\t%d kB\nRssAnon:\t%d kB\nRssFile:\t3000 kB\n"
            "RssShmem:\t%d kB\nThreads:\t1\n" % (uid, uid, uid, uid, rss_kb, anon_kb, shmem_kb))


def make_proc(root, pid, comm, uid=1000, cgroup="/user.slice", argv=None, io=(0, 0),
              **kw):
    d = root / str(pid)
    d.mkdir(parents=True, exist_ok=True)
    (d / "stat").write_text(stat_line(pid, comm, **kw))
    (d / "status").write_text(status_text(uid))
    (d / "cgroup").write_text("0::%s\n" % cgroup)
    (d / "cmdline").write_bytes(b"\0".join(a.encode() for a in (argv or [comm])) + b"\0")
    (d / "io").write_text("rchar: 10\nwchar: 10\nsyscr: 1\nsyscw: 1\nread_bytes: %d\n"
                          "write_bytes: %d\ncancelled_write_bytes: 0\n" % io)
    return d


def proc_stat_text(cores=2, busy=0, idle=0):
    lines = ["cpu  %d 0 0 %d 0 0 0 0 0 0" % (busy * cores, idle * cores)]
    for i in range(cores):
        lines.append("cpu%d %d 0 0 %d 0 0 0 0 0 0" % (i, busy, idle))
    lines += ["intr 0", "ctxt 100", "btime 1700000000", "processes 50",
              "procs_running 1", "procs_blocked 0"]
    return "\n".join(lines) + "\n"


class Clock:
    def __init__(self):
        self.t = 100.0

    def __call__(self):
        return self.t


# ------------------------------------------------------------------ parsers

def test_parse_stat_comm_with_spaces_and_parens():
    line = stat_line(42, "Web Content (x) )y", utime=7, stime=3, nice=19, threads=5,
                     start=12345)
    st = tp.parse_stat(line)
    assert st.pid == 42
    assert st.comm == "Web Content (x) )y"
    assert st.state == "S"
    assert (st.utime, st.stime, st.nice, st.threads, st.start) == (7, 3, 19, 5, 12345)
    assert st.ticks == 10
    assert not st.kernel


def test_parse_stat_kernel_thread():
    assert tp.parse_stat(stat_line(9, "kworker/0:1", ppid=2)).kernel
    assert tp.parse_stat(stat_line(9, "x", ppid=50, flags=tp.PF_KTHREAD)).kernel


def test_parse_status_memory():
    s = tp.parse_status(status_text(1000, anon_kb=2000, shmem_kb=100))
    assert s["uid"] == 1000
    assert s["private"] == 2100 * 1024
    assert s["vm_rss"] == 5000 * 1024


def test_parse_io():
    assert tp.parse_io("rchar: 5\nwchar: 6\nread_bytes: 4096\nwrite_bytes: 8192\n"
                       "cancelled_write_bytes: 0\n") == (4096, 8192)


def test_parse_meminfo():
    m = tp.parse_meminfo("MemTotal:       16000000 kB\nMemAvailable:    4000000 kB\n"
                         "HugePages_Total:       0\n")
    assert m["MemTotal"] == 16000000 * 1024
    assert m["HugePages_Total"] == 0


def test_parse_diskstats_and_rates():
    a = tp.parse_diskstats(" 259 0 nvme0n1 100 0 2000 50 200 0 4000 70 0 300 400 0 0 0 0\n"
                           "   7 0 loop0 1 0 2 0 0 0 0 0 0 0 0\n")
    b = tp.parse_diskstats(" 259 0 nvme0n1 110 0 4048 60 210 0 6048 80 0 800 900 0 0 0 0\n")
    assert "nvme0n1" in a
    active, rd, wr, resp = tp.disk_rates(a["nvme0n1"], b["nvme0n1"], 1.0)
    assert active == pytest.approx(50.0)  # 500 ms of 1000 ms busy
    assert rd == 2048 * 512
    assert wr == 2048 * 512
    assert resp == pytest.approx(20 / 20)


def test_parse_netdev():
    text = ("Inter-|   Receive                            |  Transmit\n"
            " face |bytes    packets errs drop fifo frame compressed multicast|bytes ...\n"
            "    lo: 100 1 0 0 0 0 0 0 100 1 0 0 0 0 0 0\n"
            "wlp0s20f3: 5000 10 0 0 0 0 0 0 7000 9 0 0 0 0 0 0\n")
    d = tp.parse_netdev(text)
    assert d["wlp0s20f3"] == (5000, 7000)
    assert d["lo"] == (100, 100)


def test_parse_cpu_stat_and_percent():
    t1, c1, extra = tp.parse_cpu_stat(proc_stat_text(2, busy=100, idle=100))
    t2, c2, _ = tp.parse_cpu_stat(proc_stat_text(2, busy=150, idle=150))
    assert len(c1) == 2
    assert extra["btime"] == 1700000000
    assert tp.cpu_percent(t1, t2) == pytest.approx(50.0)
    assert tp.cpu_percent(c1[0], c2[0]) == pytest.approx(50.0)


def test_process_cpu_percent_windows_style():
    # 50 ticks in 1 s at 100 Hz on 2 cores = 25 % of the whole machine
    assert tp.process_cpu_percent(0, 50, 1.0, 2, hz=100) == pytest.approx(25.0)
    assert tp.process_cpu_percent(None, 50, 1.0, 2, hz=100) == 0.0


def test_parse_cpuinfo():
    text = ""
    for i in range(4):
        text += ("processor\t: %d\nmodel name\t: Test CPU 9000\nphysical id\t: 0\n"
                 "core id\t\t: %d\nflags\t\t: fpu vmx sse\n\n" % (i, i // 2))
    info = tp.parse_cpuinfo(text)
    assert info == {"model": "Test CPU 9000", "sockets": 1, "cores": 2, "logical": 4,
                    "virtualization": "VT-x"}


def test_parse_cmdline_and_display_name():
    assert tp.parse_cmdline(b"/usr/bin/foo\0--bar\0") == ["/usr/bin/foo", "--bar"]
    assert tp.display_name("gnome-shell-cal", ["/usr/libexec/gnome-shell-calendar-server"]) \
        == "gnome-shell-calendar-server"
    # Electron rewrites cmdline into one string with spaces
    assert tp.display_name("electron-helper", ["/opt/app/electron-helper-gpu --type=gpu"]) \
        == "electron-helper-gpu"
    assert tp.display_name("bash", ["-bash"]) == "bash"


# ------------------------------------------------------------------ cgroups

@pytest.mark.parametrize("unit,app_id,launcher", [
    ("app-gnome-org.gnome.Ptyxis-12345.scope", "org.gnome.Ptyxis", "gnome"),
    ("app-gnome-google\\x2dchrome-999.scope", "google-chrome", "gnome"),
    ("app-flatpak-org.mozilla.firefox-4242.scope", "org.mozilla.firefox", "flatpak"),
    ("app-com.anthropic.Claude-9423.scope", "com.anthropic.Claude", None),
    ("app-meshdrop-7298.scope", "meshdrop", None),
    ("app-gnome-xdg\\x2duser\\x2ddirs@autostart.service", "xdg-user-dirs", "gnome"),
    ("app-org.gnome.Evolution\\x2dalarm\\x2dnotify@autostart.service",
     "org.gnome.Evolution-alarm-notify", None),
    ("app-gnome-org.gnome.Software.service", "org.gnome.Software", "gnome"),
    ("snap.brave.brave-09a27754-2464-432e-bedb-d2342818f9f7.scope", "brave", "snap"),
])
def test_app_from_unit(unit, app_id, launcher):
    info = tp.app_from_unit(unit)
    assert info is not None
    assert info["id"] == app_id
    assert info["launcher"] == launcher


def test_snap_desktop_candidates():
    info = tp.app_from_unit("snap.brave.brave-09a27754-2464-432e-bedb-d2342818f9f7.scope")
    assert info["desktop"][0] == "brave_brave.desktop"


@pytest.mark.parametrize("unit", ["dbus.service", "session-2.scope", "init.scope",
                                  "app.slice", "gcr-ssh-agent.service",
                                  "snap.snapd-desktop-integration.snapd-desktop-integration"
                                  ".service"])
def test_not_an_app(unit):
    assert tp.app_from_unit(unit) is None


def test_app_of_cgroup_and_unit_of():
    cg = "/user.slice/user-1000.slice/user@1000.service/app.slice/app-gnome-org.x.Y-5.scope"
    info, unit = tp.app_of_cgroup(cg)
    assert info["id"] == "org.x.Y" and unit == "app-gnome-org.x.Y-5.scope"
    assert tp.app_of_cgroup("/user.slice/user-1000.slice/user@1000.service/session.slice/"
                            "org.gnome.Shell@ubuntu.service") is None
    assert tp.unit_of("/system.slice/cron.service") == "cron.service"
    assert tp.uid_of_user_slice(cg) == 1000
    assert tp.parse_cgroup(b"0::/system.slice/x.service\n") == "/system.slice/x.service"


# ------------------------------------------------------------------ sampler + grouping

APP_CG = "/user.slice/user-1000.slice/user@1000.service/app.slice/app-gnome-org.ex.Edit-77.scope"
BG_CG = "/user.slice/user-1000.slice/user@1000.service/session.slice/dbus.service"
SYS_CG = "/system.slice/cron.service"


def build_tree(tmp_path):
    root = tmp_path / "proc"
    root.mkdir()
    (root / "stat").write_text(proc_stat_text(2))
    make_proc(root, 100, "edit", cgroup=APP_CG, argv=["/usr/bin/edit"])
    make_proc(root, 101, "edit-helper", cgroup=APP_CG, ppid=100)
    make_proc(root, 200, "dbus-daemon", cgroup=BG_CG)
    make_proc(root, 201, "dbus-launch", cgroup=BG_CG)
    make_proc(root, 300, "cron", uid=0, cgroup=SYS_CG)
    make_proc(root, 3, "kworker/0:0", uid=0, ppid=2, cgroup="/")
    return root


def test_sampler_cpu_memory_disk(tmp_path):
    root = build_tree(tmp_path)
    clock = Clock()
    s = tp.ProcSampler(root=str(root), ncpu=2, my_uid=1000, clock=clock, hz=100)
    procs = s.sample()
    assert set(procs) == {100, 101, 200, 201, 300, 3}
    assert procs[100].mem == (1000 + 24) * 1024
    assert procs[100].uid == 1000
    assert procs[300].uid == 0
    # one second later edit used 50 ticks and wrote 1 MB
    make_proc(root, 100, "edit", cgroup=APP_CG, utime=40, stime=10, io=(0, 1048576), rss=200)
    clock.t += 1.0
    procs = s.sample()
    p = procs[100]
    assert p.cpu == pytest.approx(25.0)
    assert p.disk == pytest.approx(1048576.0)
    assert p.cpu_seconds == pytest.approx(0.5)
    assert procs[200].cpu == 0.0
    # a process that does not use CPU is not re-read for io
    assert procs[200].disk == 0.0


def test_sampler_pid_reuse_is_a_new_process(tmp_path):
    root = build_tree(tmp_path)
    s = tp.ProcSampler(root=str(root), ncpu=2, my_uid=1000, clock=Clock(), hz=100)
    first = s.sample()[200]
    make_proc(root, 200, "other", cgroup=BG_CG, start=99999)
    second = s.sample()[200]
    assert second is not first
    assert second.name == "other"
    assert first in s.removed and second in s.added


def test_classify(tmp_path):
    root = build_tree(tmp_path)
    s = tp.ProcSampler(root=str(root), ncpu=2, my_uid=1000, clock=Clock(), hz=100)
    procs = s.sample()
    assert tp.classify(procs[100], 1000) == ("apps", "org.ex.Edit")
    assert tp.classify(procs[200], 1000) == ("background", "dbus.service")
    assert tp.classify(procs[300], 1000) == ("system", "cron.service")
    assert tp.classify(procs[3], 1000) == ("kernel", None)


def test_descendants(tmp_path):
    root = build_tree(tmp_path)
    make_proc(root, 102, "edit-grandchild", cgroup=APP_CG, ppid=101)
    s = tp.ProcSampler(root=str(root), ncpu=2, my_uid=1000, clock=Clock(), hz=100)
    procs = s.sample()
    assert sorted(tp.descendants(procs, 100)) == [101, 102]


def test_models_grouping(tmp_path):
    pytest.importorskip("gi")
    from harbor.taskmgr_model import ProcModels

    class Lookup:
        def __call__(self, app):
            return ("Editor", None, True)
    root = build_tree(tmp_path)
    s = tp.ProcSampler(root=str(root), ncpu=2, my_uid=1000, clock=Clock(), hz=100)
    m = ProcModels(Lookup(), my_uid=1000)
    m.update(s.sample(), 16 * 1024 ** 3)
    apps = m.groups["apps"].children
    assert apps.get_n_items() == 1
    app = apps.get_item(0)
    assert app.kind == "app" and app.label == "Editor (2)" and app.count == 2
    assert app.children.get_n_items() == 2
    bg = m.groups["background"].children
    assert bg.get_n_items() == 1 and bg.get_item(0).label == "dbus-daemon (2)"
    sysg = m.groups["system"].children
    kinds = sorted(sysg.get_item(i).kind for i in range(sysg.get_n_items()))
    assert kinds == ["kernel", "proc"]
    assert m.groups["apps"].label == "Apps (1)"
    assert m.details.get_n_items() == 5  # kernel threads are not listed
    # in-place update: same item objects after another sample
    before = app
    m.update(s.sample(), 16 * 1024 ** 3)
    assert m.groups["apps"].children.get_item(0) is before
    # search
    m.set_query("cron")
    assert m.items[(300, 1000)].match
    assert not m.items[(100, 1000)].match
    m.set_query("101")
    assert m.items[(101, 1000)].match
    # group by type off puts every top-level row in the root
    m.set_query("")
    m.set_group_by_type(False)
    assert m.root.get_n_items() == 4
    users = [m.users_root.get_item(i).key for i in range(m.users_root.get_n_items())]
    assert users == [1000]


# ------------------------------------------------------------------ guards and actions

@pytest.fixture
def sleeper():
    p = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
    time.sleep(0.2)
    yield p
    if p.poll() is None:
        p.kill()
    p.wait()


def test_pid_reuse_guard(sleeper):
    start = tp.read_start(sleeper.pid)
    assert start is not None
    assert tp.same_process(sleeper.pid, start)
    assert not tp.same_process(sleeper.pid, start + 1)
    # a wrong start time must never signal
    assert not tp.send_signal(sleeper.pid, start + 1, signal.SIGTERM, dry_run=False)
    assert sleeper.poll() is None


def test_send_signal_dry_run_logs_only(sleeper):
    start = tp.read_start(sleeper.pid)
    tp.ACTION_LOG.clear()
    assert tp.send_signal(sleeper.pid, start, signal.SIGTERM, dry_run=True)
    assert tp.ACTION_LOG == [["kill", "-15", str(sleeper.pid)]]
    time.sleep(0.1)
    assert sleeper.poll() is None


def test_send_signal_real_on_own_child(sleeper):
    start = tp.read_start(sleeper.pid)
    assert tp.alive(sleeper.pid, start)
    assert tp.send_signal(sleeper.pid, start, signal.SIGTERM, dry_run=False)
    sleeper.wait(timeout=5)
    assert not tp.alive(sleeper.pid, start)


def test_never_signal_pid1_or_self():
    assert not tp.send_signal(1, tp.read_start(1), signal.SIGTERM, dry_run=True)
    assert not tp.send_signal(os.getpid(), tp.read_start(os.getpid()), signal.SIGTERM,
                              dry_run=True)


def test_set_nice_on_own_child(sleeper):
    start = tp.read_start(sleeper.pid)
    assert tp.set_nice(sleeper.pid, start, 19, dry_run=False) == "ok"
    assert os.getpriority(os.PRIO_PROCESS, sleeper.pid) == 19
    # raising the priority again needs root
    assert tp.set_nice(sleeper.pid, start, 0, dry_run=False) == "need-root"
    assert tp.set_nice(sleeper.pid, start + 1, 19, dry_run=False) == "gone"


def test_nice_mapping():
    assert [n for _l, n in tp.NICE_LEVELS] == [-10, -5, 0, 5, 19]
    assert tp.nice_for("Below normal") == 5
    assert tp.priority_name(-15) == "High"
    assert tp.priority_name(-5) == "Above normal"
    assert tp.priority_name(0) == "Normal"
    assert tp.priority_name(5) == "Below normal"
    assert tp.priority_name(19) == "Low"
    with pytest.raises(KeyError):
        tp.nice_for("Realtime")


# ------------------------------------------------------------------ system sampler

def test_system_sampler(tmp_path):
    proc = tmp_path / "proc"
    (proc / "net").mkdir(parents=True)
    sysr = tmp_path / "sys"
    for d in ("block/nvme0n1/queue", "block/nvme0n1/device", "block/loop0",
              "class/net/eth0/device", "class/net/wlan0/device", "class/net/wlan0/wireless",
              "class/net/docker0", "class/net/lo"):
        (sysr / d).mkdir(parents=True)
    (sysr / "block/nvme0n1/size").write_text("2000000\n")
    (sysr / "block/nvme0n1/queue/rotational").write_text("0\n")
    (sysr / "block/nvme0n1/device/model").write_text("Fast SSD  \n")
    (proc / "stat").write_text(proc_stat_text(2, 100, 100))
    (proc / "meminfo").write_text("MemTotal: 1000 kB\nMemFree: 100 kB\nMemAvailable: 400 kB\n"
                                  "Cached: 300 kB\nSReclaimable: 0 kB\nShmem: 0 kB\n"
                                  "Committed_AS: 900 kB\nCommitLimit: 2000 kB\n"
                                  "SwapTotal: 0 kB\nSwapFree: 0 kB\n")
    (proc / "diskstats").write_text(" 259 0 nvme0n1 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0\n")
    (proc / "net/dev").write_text("h\nh\n eth0: 1000 0 0 0 0 0 0 0 2000 0 0 0 0 0 0 0\n"
                                  " wlan0: 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n")
    (proc / "cpuinfo").write_text("processor : 0\nmodel name : X\n\n")
    clock = Clock()
    s = tp.SystemSampler(str(proc), str(sysr), clock=clock)
    assert [d["name"] for d in s.disks] == ["nvme0n1"]
    assert s.disks[0]["model"] == "Fast SSD" and s.disks[0]["type"] == "SSD (NVMe)"
    assert s.nets == [("eth0", "Ethernet"), ("wlan0", "Wi-Fi")]
    s.sample()
    (proc / "stat").write_text(proc_stat_text(2, 175, 125))
    (proc / "diskstats").write_text(" 259 0 nvme0n1 1 0 2048 0 1 0 0 0 0 250 0 0 0 0 0\n")
    (proc / "net/dev").write_text("h\nh\n eth0: 126000 0 0 0 0 0 0 0 2000 0 0 0 0 0 0 0\n"
                                  " wlan0: 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n")
    clock.t += 1.0
    s.sample()
    assert s.cpu == pytest.approx(75.0)
    assert s.mem_used_fraction() == pytest.approx(0.6)
    assert s.disk_now["nvme0n1"][0] == pytest.approx(25.0)
    assert s.disk_now["nvme0n1"][1] == 2048 * 512
    assert s.net_now["eth0"] == (125000.0, 0.0)
    assert s.net_total() == 125000.0
    m = s.mem_summary()
    assert m["used"] == 600 * 1024 and m["committed"] == 900 * 1024


def test_parse_drm_fdinfo_and_sockets():
    info = tp.parse_drm_fdinfo("pos: 0\ndrm-driver:\ti915\ndrm-client-id:\t27\n"
                               "drm-pdev:\t0000:00:02.0\ndrm-engine-render:\t500 ns\n"
                               "drm-engine-copy:\t0 ns\n")
    assert info == ("27", "0000:00:02.0", {"render": 500, "copy": 0})
    assert tp.parse_drm_fdinfo("pos: 0\nflags: 1\n") is None
    tcp = ("  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt"
           "   uid  timeout inode\n"
           "   0: 0100007F:0277 00000000:0000 0A 00000000:00000000 00:00000000 00000000"
           "     0        0 111 1\n"
           "   1: 0100007F:9C40 0100007F:0277 01 00000000:00000000 00:00000000 00000000"
           "  1000        0 222 1\n")
    assert tp.parse_net_sockets(tcp) == {111: False, 222: True}


def test_nvidia_smi_parse():
    rows = tp.parse_nvidia_smi("0, NVIDIA GeForce RTX 3060, 37, 1200, 12288, 1500\n")
    assert rows[0]["util"] == 37.0 and rows[0]["mem_total"] == 12288.0
    assert rows[0]["clock"] == 1500


# ------------------------------------------------------------------ autostart

def test_autostart_list_and_toggle(tmp_path):
    user = tmp_path / "user"
    system = tmp_path / "xdg"
    system.mkdir()
    (system / "foo.desktop").write_text(
        "[Desktop Entry]\nType=Application\nName=Foo\nExec=foo\nIcon=foo\n"
        "X-GNOME-Bugzilla-Bugzilla=GNOME\n\n[Desktop Action x]\nName=X\n")
    (system / "kdeonly.desktop").write_text(
        "[Desktop Entry]\nType=Application\nName=K\nExec=k\nOnlyShowIn=KDE;\n")
    (system / "notgnome.desktop").write_text(
        "[Desktop Entry]\nType=Application\nName=N\nExec=n\nNotShowIn=GNOME;\n")
    (system / "hidden.desktop").write_text(
        "[Desktop Entry]\nType=Application\nName=H\nExec=h\nHidden=true\n")
    (system / "org.example.Bar.desktop").write_text(
        "[Desktop Entry]\nType=Application\nName=Bar\nExec=bar\n"
        "X-GNOME-Autostart-enabled=false\n")
    entries = {e["id"]: e for e in ts.list_autostart(str(user), [str(system)], ["GNOME"])}
    assert set(entries) == {"foo.desktop", "org.example.Bar.desktop"}
    assert entries["foo.desktop"]["enabled"] and entries["foo.desktop"]["publisher"] == "GNOME"
    assert not entries["org.example.Bar.desktop"]["enabled"]
    assert entries["org.example.Bar.desktop"]["publisher"] == "example.org"
    # disable writes a user copy and never touches the system file
    sys_before = (system / "foo.desktop").read_text()
    path = ts.set_autostart_enabled(entries["foo.desktop"], False, str(user))
    assert path == str(user / "foo.desktop")
    assert (system / "foo.desktop").read_text() == sys_before
    text = (user / "foo.desktop").read_text()
    assert "X-GNOME-Autostart-enabled=false" in text
    # the key lands in [Desktop Entry], not in the action group
    assert text.index("X-GNOME-Autostart-enabled") < text.index("[Desktop Action x]")
    entries = {e["id"]: e for e in ts.list_autostart(str(user), [str(system)], ["GNOME"])}
    assert not entries["foo.desktop"]["enabled"]
    ts.set_autostart_enabled(entries["foo.desktop"], True, str(user))
    entries = {e["id"]: e for e in ts.list_autostart(str(user), [str(system)], ["GNOME"])}
    assert entries["foo.desktop"]["enabled"]
    # a user override with Hidden=true counts as disabled and enabling clears it
    (user / "org.example.Bar.desktop").write_text(
        "[Desktop Entry]\nType=Application\nName=Bar\nExec=bar\nHidden=true\n")
    e = {x["id"]: x for x in ts.list_autostart(str(user), [str(system)])}[
        "org.example.Bar.desktop"]
    assert not e["enabled"]
    ts.set_autostart_enabled(e, True, str(user))
    e = {x["id"]: x for x in ts.list_autostart(str(user), [str(system)])}[
        "org.example.Bar.desktop"]
    assert e["enabled"]


def test_startup_impact_rule():
    pytest.importorskip("gi")
    from harbor.taskmgr_pages import startup_impact
    assert startup_impact(2.0, 0) == "High"
    assert startup_impact(0, 4 * 1024 * 1024) == "High"
    assert startup_impact(0.5, 0) == "Medium"
    assert startup_impact(0.1, 1000) == "Low"


# ------------------------------------------------------------------ systemd

def test_parse_units_json():
    text = json.dumps([
        {"unit": "cron.service", "load": "loaded", "active": "active", "sub": "running",
         "description": "Regular background program processing daemon"},
        {"unit": "apport.service", "load": "loaded", "active": "active", "sub": "exited",
         "description": "crash reports"},
        {"unit": "bad.service", "load": "loaded", "active": "failed", "sub": "failed",
         "description": "Bad"},
        {"unit": "old.service", "load": "not-found", "active": "inactive", "sub": "dead",
         "description": "old"},
        {"unit": "x.socket", "load": "loaded", "active": "active", "sub": "listening",
         "description": "not a service"},
    ])
    rows = ts.parse_units_json(text, "user")
    assert [r["name"] for r in rows] == ["cron.service", "apport.service", "bad.service",
                                         "old.service"]
    assert [r["status"] for r in rows] == ["Running", "Exited", "Failed", "Stopped"]
    assert rows[0]["scope"] == "user"
    assert ts.parse_units_json("not json") == []


def test_parse_show_and_argv():
    text = "Id=dbus.service\nMainPID=1528\n\nMainPID=0\nId=cron.service\n"
    assert ts.parse_show(text) == {"dbus.service": 1528, "cron.service": 0}
    assert ts.service_argv("restart", "x.service", "user") == \
        ["systemctl", "--user", "restart", "x.service"]
    assert ts.service_argv("stop", "x.service", "system") == ["systemctl", "stop", "x.service"]


# ------------------------------------------------------------------ persistence

def test_history_persistence(tmp_path):
    path = str(tmp_path / "harbor" / "app-history.json")
    h = ts.AppHistory(path, clock=lambda: 1000.0)
    h.add("org.ex.Edit", "Editor", 1.5, 4096, icon="edit")
    h.add("org.ex.Edit", "Editor", 0.5, 4096)
    h.save()
    h2 = ts.AppHistory(path, clock=lambda: 2000.0)
    assert h2.since == 1000.0
    assert h2.apps["org.ex.Edit"]["cpu"] == pytest.approx(2.0)
    assert h2.apps["org.ex.Edit"]["disk"] == 8192
    assert h2.apps["org.ex.Edit"]["icon"] == "edit"
    h2.clear()
    h3 = ts.AppHistory(path)
    assert h3.apps == {} and h3.since == 2000.0
    assert ts.format_cpu_time(3725) == "1:02:05"


def test_settings_persistence(tmp_path):
    path = str(tmp_path / "cfg" / "taskmgr.json")
    s = ts.Settings(path)
    assert s["interval"] == "Normal" and s.interval_ms() == 1000
    s["interval"] = "Low"
    s["start_page"] = "details"
    s2 = ts.Settings(path)
    assert s2.interval_ms() == 4000 and s2["start_page"] == "details"
    s2["interval"] = "Paused"
    assert ts.Settings(path).interval_ms() == 0
    # a broken file falls back to defaults
    with open(path, "w") as f:
        f.write("{broken")
    assert ts.Settings(path)["interval"] == "Normal"
