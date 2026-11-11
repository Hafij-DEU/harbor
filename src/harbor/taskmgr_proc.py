"""Process and system samplers for Harbor Task Manager.

Pure Python, no GTK: everything reads /proc and /sys under a root path so
the tests can point it at a fake tree. The samplers keep their previous
readings and only compute deltas, and they avoid re-reading files whose
content cannot have changed (status only when RSS moved, io only when the
process used CPU, cmdline/exe once per exec).
"""

import os
import pwd
import re
import signal
import time

CLK_TCK = os.sysconf("SC_CLK_TCK")
PAGE_SIZE = os.sysconf("SC_PAGE_SIZE")
PF_KTHREAD = 0x00200000

DRY_RUN = os.environ.get("HARBOR_DRY_RUN") == "1"
ACTION_LOG = []  # what dry-run actions would have done (tests read it)


# ------------------------------------------------------------------ file helpers

def read_bytes(path, small=False):
    """Whole file as bytes, or None if it cannot be read.

    /proc seq files can return short reads, so keep reading until EOF;
    small=True does a single read for files that are always tiny
    (/proc/PID/stat, io), which saves a syscall per process per tick."""
    try:
        fd = os.open(path, os.O_RDONLY | os.O_CLOEXEC)
    except OSError:
        return None
    try:
        data = os.read(fd, 4096)
        if small or not data:
            return data
        parts = [data]
        while True:
            more = os.read(fd, 65536)
            if not more:
                break
            parts.append(more)
        return b"".join(parts) if len(parts) > 1 else data
    except OSError:
        return None
    finally:
        os.close(fd)


def read_text(path, default=""):
    data = read_bytes(path)
    if data is None:
        return default
    return data.decode("utf-8", "replace")


def read_int(path, default=None):
    data = read_bytes(path)
    if not data:
        return default
    try:
        return int(data.split()[0])
    except (ValueError, IndexError):
        return default


# ------------------------------------------------------------------ parsers

class Stat:
    """The fields of /proc/PID/stat Task Manager needs."""
    __slots__ = ("pid", "comm", "state", "ppid", "flags", "utime", "stime", "nice",
                 "threads", "start", "rss")

    def __init__(self, pid, comm, state, ppid, flags, utime, stime, nice, threads, start, rss):
        self.pid = pid
        self.comm = comm
        self.state = state
        self.ppid = ppid
        self.flags = flags
        self.utime = utime
        self.stime = stime
        self.nice = nice
        self.threads = threads
        self.start = start
        self.rss = rss

    @property
    def ticks(self):
        return self.utime + self.stime

    @property
    def kernel(self):
        return bool(self.flags & PF_KTHREAD) or self.pid == 2 or self.ppid == 2


def parse_stat(data):
    """Parse /proc/PID/stat. comm may hold spaces and parentheses, so it is
    everything between the first '(' and the LAST ')'."""
    if isinstance(data, str):
        data = data.encode()
    lp = data.find(b"(")
    rp = data.rfind(b")")
    if lp < 0 or rp < lp:
        raise ValueError("bad stat line")
    pid = int(data[:lp])
    comm = data[lp + 1:rp].decode("utf-8", "replace")
    f = data[rp + 2:].split()
    # f[0] is field 3 (state)
    return Stat(pid, comm, f[0].decode(), int(f[1]), int(f[6]), int(f[11]), int(f[12]),
                int(f[16]), int(f[17]), int(f[19]), int(f[21]))


def _kb_field(data, key):
    i = data.find(key)
    if i < 0:
        return 0
    j = data.find(b"\n", i + len(key))
    try:
        return int(data[i + len(key):j if j > 0 else None].split()[0])
    except (ValueError, IndexError):
        return 0


def parse_status(data):
    """Uid and the memory fields of /proc/PID/status (sizes in bytes)."""
    if isinstance(data, str):
        data = data.encode()
    uid = None
    i = data.find(b"\nUid:")
    if i >= 0:
        try:
            uid = int(data[i + 5:data.find(b"\n", i + 1)].split()[0])
        except (ValueError, IndexError):
            uid = None
    anon = _kb_field(data, b"RssAnon:")
    shmem = _kb_field(data, b"RssShmem:")
    return {"uid": uid, "rss_anon": anon * 1024, "rss_shmem": shmem * 1024,
            "vm_rss": _kb_field(data, b"VmRSS:") * 1024,
            "private": (anon + shmem) * 1024}


def parse_io(data):
    """read_bytes + write_bytes of /proc/PID/io."""
    if isinstance(data, str):
        data = data.encode()
    return _kb_field(data, b"\nread_bytes:"), _kb_field(data, b"\nwrite_bytes:")


def parse_meminfo(text):
    """{key: bytes} for /proc/meminfo (kB values converted to bytes)."""
    out = {}
    for line in text.splitlines():
        key, _, rest = line.partition(":")
        parts = rest.split()
        if not parts:
            continue
        try:
            val = int(parts[0])
        except ValueError:
            continue
        if len(parts) > 1 and parts[1] == "kB":
            val *= 1024
        out[key.strip()] = val
    return out


def parse_cpu_stat(text):
    """(total, [per cpu]) where each is (busy_jiffies, total_jiffies), plus
    a dict of the scalar lines (procs_running, btime, ...)."""
    total = None
    cores = []
    extra = {}
    for line in text.splitlines():
        if line.startswith("cpu"):
            parts = line.split()
            vals = [int(v) for v in parts[1:]]
            # user nice system idle iowait irq softirq steal guest guest_nice
            # guest time is already counted in user/nice
            tot = sum(vals[:8])
            idle = vals[3] + (vals[4] if len(vals) > 4 else 0)
            pair = (tot - idle, tot)
            if parts[0] == "cpu":
                total = pair
            else:
                cores.append(pair)
        else:
            parts = line.split()
            if len(parts) == 2 and parts[1].isdigit():
                extra[parts[0]] = int(parts[1])
    return total or (0, 0), cores, extra


def cpu_percent(prev, cur):
    """Busy % between two (busy, total) jiffy pairs."""
    if prev is None or cur is None:
        return 0.0
    dt = cur[1] - prev[1]
    if dt <= 0:
        return 0.0
    return max(0.0, min(100.0, 100.0 * (cur[0] - prev[0]) / dt))


def process_cpu_percent(prev_ticks, ticks, elapsed, ncpu, hz=CLK_TCK):
    """Windows-style process CPU %: 100% means every core busy."""
    if elapsed <= 0 or ncpu <= 0 or prev_ticks is None:
        return 0.0
    return max(0.0, 100.0 * (ticks - prev_ticks) / (elapsed * hz * ncpu))


class DiskStat:
    __slots__ = ("name", "rd_ios", "rd_sectors", "rd_ticks", "wr_ios", "wr_sectors",
                 "wr_ticks", "io_ticks")

    def __init__(self, name, rd_ios, rd_sectors, rd_ticks, wr_ios, wr_sectors, wr_ticks,
                 io_ticks):
        self.name = name
        self.rd_ios = rd_ios
        self.rd_sectors = rd_sectors
        self.rd_ticks = rd_ticks
        self.wr_ios = wr_ios
        self.wr_sectors = wr_sectors
        self.wr_ticks = wr_ticks
        self.io_ticks = io_ticks


def parse_diskstats(text):
    out = {}
    for line in text.splitlines():
        p = line.split()
        if len(p) < 14:
            continue
        try:
            out[p[2]] = DiskStat(p[2], int(p[3]), int(p[5]), int(p[6]), int(p[7]),
                                 int(p[9]), int(p[10]), int(p[12]))
        except ValueError:
            continue
    return out


def disk_rates(prev, cur, elapsed):
    """(active %, read B/s, write B/s, avg response ms) between two DiskStats."""
    if prev is None or elapsed <= 0:
        return 0.0, 0.0, 0.0, 0.0
    active = max(0.0, min(100.0, (cur.io_ticks - prev.io_ticks) / (elapsed * 10.0)))
    rd = max(0, cur.rd_sectors - prev.rd_sectors) * 512 / elapsed
    wr = max(0, cur.wr_sectors - prev.wr_sectors) * 512 / elapsed
    ios = (cur.rd_ios - prev.rd_ios) + (cur.wr_ios - prev.wr_ios)
    ticks = (cur.rd_ticks - prev.rd_ticks) + (cur.wr_ticks - prev.wr_ticks)
    resp = ticks / ios if ios > 0 else 0.0
    return active, rd, wr, resp


def parse_netdev(text):
    """{iface: (rx_bytes, tx_bytes)} from /proc/net/dev."""
    out = {}
    for line in text.splitlines():
        if ":" not in line:
            continue
        name, _, rest = line.partition(":")
        name = name.strip()
        p = rest.split()
        if len(p) < 9 or not name:
            continue
        try:
            out[name] = (int(p[0]), int(p[8]))
        except ValueError:
            continue
    return out


def parse_cgroup(data):
    """The cgroup v2 path ("0::/...") of /proc/PID/cgroup."""
    if isinstance(data, bytes):
        data = data.decode("utf-8", "replace")
    first = ""
    for line in data.splitlines():
        if line.startswith("0::"):
            return line[3:]
        if not first and line.count(":") >= 2:
            first = line.split(":", 2)[2]
    return first


def parse_cmdline(data):
    """argv list from /proc/PID/cmdline."""
    if not data:
        return []
    return [a.decode("utf-8", "replace") for a in data.rstrip(b"\0").split(b"\0")]


def parse_cpuinfo(text):
    """Model, sockets, cores and logical processors from /proc/cpuinfo."""
    model = ""
    sockets = set()
    cores = set()
    logical = 0
    flags = ""
    phys = core = None
    for line in text.splitlines() + [""]:
        if not line.strip():
            if phys is not None or core is not None:
                cores.add((phys, core))
            phys = core = None
            continue
        key, _, val = line.partition(":")
        key = key.strip()
        val = val.strip()
        if key == "processor":
            logical += 1
        elif key == "model name" and not model:
            model = val
        elif key == "physical id":
            phys = val
            sockets.add(val)
        elif key == "core id":
            core = val
        elif key == "flags" and not flags:
            flags = val
    return {"model": model, "sockets": len(sockets) or 1,
            "cores": len(cores) or logical, "logical": logical,
            "virtualization": "VT-x" if " vmx" in " " + flags else
            ("AMD-V" if " svm" in " " + flags else "")}


# ------------------------------------------------------------------ cgroups and apps

_ESC = re.compile(r"\\x([0-9a-fA-F]{2})")
_UUID = r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"
_SNAP = re.compile(r"^snap\.([^.]+)\.(.+?)[.-](" + _UUID + r"|\d+)\.scope$")
LAUNCHERS = {"gnome", "flatpak", "kde", "xfce", "sway", "niri", "hyprland", "cosmic",
             "systemd", "dbus", "budgie", "cinnamon", "mate", "lxqt"}


def systemd_unescape(s):
    return _ESC.sub(lambda m: chr(int(m.group(1), 16)), s)


def app_from_unit(unit):
    """Application identity of an app unit name, or None.

    Handles the XDG/systemd naming scheme
      app[-<launcher>]-<ID>-<RANDOM>.scope   (app-gnome-org.gnome.Ptyxis-1234.scope,
                                              app-flatpak-org.foo.Bar-99.scope)
      app[-<launcher>]-<ID>[@<RANDOM>].service (app-gnome-foo@autostart.service)
      snap.<name>.<app>-<uuid>.scope
    Returns {"id", "launcher", "kind" ("scope"/"service"), "desktop" [candidates]}.
    """
    m = _SNAP.match(unit)
    if m:
        name, app = m.group(1), m.group(2)
        return {"id": name, "launcher": "snap", "kind": "scope",
                "desktop": ["%s_%s.desktop" % (name, app), "%s.desktop" % name]}
    if not unit.startswith("app-"):
        return None
    if unit.endswith(".scope"):
        core = unit[4:-6]
        parts = core.split("-")
        if len(parts) < 2:
            return None
        parts = parts[:-1]
        kind = "scope"
    elif unit.endswith(".service"):
        core = unit[4:-8]
        core = core.split("@", 1)[0]
        parts = core.split("-")
        kind = "service"
    else:
        return None
    launcher = None
    if len(parts) >= 2 and parts[0] in LAUNCHERS:
        launcher = parts[0]
        parts = parts[1:]
    app_id = systemd_unescape("-".join(parts))
    if not app_id:
        return None
    cands = [app_id + ".desktop"]
    if app_id.lower() != app_id:
        cands.append(app_id.lower() + ".desktop")
    return {"id": app_id, "launcher": launcher, "kind": kind, "desktop": cands}


def unit_of(cgroup):
    """Innermost systemd unit (.service / .scope) of a cgroup path."""
    for comp in reversed(cgroup.split("/")):
        if comp.endswith((".service", ".scope")):
            return comp
    return cgroup.rsplit("/", 1)[-1] or "-.slice"


def app_of_cgroup(cgroup):
    """(app dict, unit) for processes inside a session app unit, else None."""
    if "app.slice" not in cgroup and "/snap." not in cgroup:
        return None
    for comp in reversed(cgroup.split("/")):
        if comp.startswith(("app-", "snap.")):
            info = app_from_unit(comp)
            if info:
                return info, comp
    return None


def uid_of_user_slice(cgroup):
    m = re.search(r"/user-(\d+)\.slice", cgroup)
    return int(m.group(1)) if m else None


def classify(proc, my_uid):
    """Task Manager group of a process: ("apps", app_id) / ("background", unit)
    / ("system", unit) / ("kernel", None)."""
    if proc.kernel:
        return "kernel", None
    if proc.uid == my_uid:
        if proc.app is not None:
            return "apps", proc.app["id"]
        return "background", proc.unit
    return "system", proc.unit


# ------------------------------------------------------------------ users

_USERS = {}


def user_info(uid):
    """(login, full name) for a uid, cached."""
    if uid is None:
        return "?", "?"
    if uid not in _USERS:
        try:
            pw = pwd.getpwuid(uid)
            full = pw.pw_gecos.split(",")[0].strip() or pw.pw_name
            _USERS[uid] = (pw.pw_name, full)
        except KeyError:
            _USERS[uid] = (str(uid), str(uid))
    return _USERS[uid]


# ------------------------------------------------------------------ processes

STATE_NAMES = {"R": "Running", "S": "Sleeping", "D": "Waiting (disk)", "T": "Stopped",
               "t": "Stopped (traced)", "Z": "Zombie", "X": "Dead", "I": "Idle",
               "P": "Parked", "W": "Paging"}


def state_name(state):
    return STATE_NAMES.get(state, state or "?")


class Proc:
    """One live process (pid + start time) and its latest readings."""
    __slots__ = ("pid", "start", "comm", "name", "argv", "exe", "uid", "state", "ppid",
                 "threads", "nice", "ticks", "cpu", "mem", "rss", "disk", "io_total",
                 "io_ok", "io_time", "mem_time", "cgroup", "unit", "app", "kernel",
                 "first_seen", "cpu_seconds", "disk_delta", "ticks_delta", "nfds",
                 "conns")

    def __init__(self, st, now):
        self.pid = st.pid
        self.start = st.start
        self.comm = st.comm
        self.name = st.comm
        self.argv = []
        self.exe = ""
        self.uid = None
        self.state = st.state
        self.ppid = st.ppid
        self.threads = st.threads
        self.nice = st.nice
        self.ticks = st.ticks
        self.cpu = 0.0
        self.mem = 0
        self.rss = -1
        self.disk = 0.0
        self.io_total = None
        self.io_ok = True
        self.io_time = now
        self.mem_time = 0.0
        self.cgroup = ""
        self.unit = ""
        self.app = None
        self.kernel = st.kernel
        self.first_seen = now
        self.cpu_seconds = 0.0
        self.disk_delta = 0
        self.ticks_delta = 0
        self.nfds = None  # open file descriptors (own processes only)
        self.conns = None  # active network connections (own processes only)

    @property
    def cmdline(self):
        return " ".join(self.argv) if self.argv else ("[%s]" % self.comm)

    @property
    def efficiency(self):
        return self.nice >= 19

    @property
    def suspended(self):
        return self.state in ("T", "t")


def display_name(comm, argv, exe=""):
    """comm is cut at 15 characters; use argv[0] or exe when they extend it."""
    cands = []
    if argv:
        a0 = argv[0]
        # Chromium/Electron rewrite their cmdline into one string with spaces
        if " " in a0 and not os.path.exists(a0):
            a0 = a0.split(" ", 1)[0]
        cands.append(os.path.basename(a0))
    if exe:
        cands.append(os.path.basename(exe).replace(" (deleted)", ""))
    if len(comm) >= 15:
        for c in cands:
            if c.startswith(comm) and len(c) > len(comm):
                return c
    return comm


class ProcSampler:
    """Reads every process under root once per sample()."""

    def __init__(self, root="/proc", ncpu=None, my_uid=None, clock=time.monotonic,
                 hz=CLK_TCK):
        self.root = root
        self.ncpu = ncpu or _count_cpus(root) or os.cpu_count() or 1
        self.my_uid = os.getuid() if my_uid is None else my_uid
        self.clock = clock
        self.hz = hz
        self.procs = {}
        self.last = None
        self.added = []
        self.removed = []

    def _path(self, pid, name):
        return "%s/%d/%s" % (self.root, pid, name)

    def _fill_identity(self, p):
        p.argv = parse_cmdline(read_bytes(self._path(p.pid, "cmdline")))
        try:
            p.exe = os.readlink(self._path(p.pid, "exe"))
        except OSError:
            p.exe = ""
        p.name = display_name(p.comm, p.argv, p.exe)
        self._fill_cgroup(p)

    def _fill_cgroup(self, p):
        p.cgroup = parse_cgroup(read_bytes(self._path(p.pid, "cgroup")) or b"")
        p.unit = unit_of(p.cgroup) if p.cgroup else ""
        found = app_of_cgroup(p.cgroup) if p.cgroup else None
        if found:
            p.app, p.unit = found
        else:
            p.app = None

    def sample(self):
        now = self.clock()
        elapsed = (now - self.last) if self.last is not None else 0.0
        self.last = now
        try:
            names = os.listdir(self.root)
        except OSError:
            names = []
        old = self.procs
        new = {}
        added = []
        for name in names:
            if not name.isdigit():
                continue
            pid = int(name)
            data = read_bytes(self._path(pid, "stat"), True)
            if not data:
                continue
            try:
                st = parse_stat(data)
            except (ValueError, IndexError):
                continue
            p = old.get(pid)
            fresh = p is None or p.start != st.start
            if fresh:
                p = Proc(st, now)
                added.append(p)
                prev_ticks = None
            else:
                prev_ticks = p.ticks
                if st.comm != p.comm:  # exec()
                    p.comm = st.comm
                    self._fill_identity(p)
                p.state = st.state
                p.ppid = st.ppid
                p.threads = st.threads
                p.nice = st.nice
                p.ticks = st.ticks
            new[pid] = p
            if p.kernel:
                p.ticks_delta = 0 if prev_ticks is None else st.ticks - prev_ticks
                p.cpu = process_cpu_percent(prev_ticks, st.ticks, elapsed, self.ncpu, self.hz)
                continue
            if fresh:
                sdata = read_bytes(self._path(pid, "status"))
                if sdata:
                    s = parse_status(sdata)
                    p.uid = s["uid"]
                    p.mem = s["private"]
                p.rss = st.rss
                p.mem_time = now
                self._fill_identity(p)
            else:
                if st.rss != p.rss or now - p.mem_time > 5.0:
                    sdata = read_bytes(self._path(pid, "status"))
                    if sdata:
                        p.mem = parse_status(sdata)["private"]
                    p.rss = st.rss
                    p.mem_time = now
                if now - p.first_seen < 5.0:
                    # just-launched apps are moved into their scope after fork
                    self._fill_cgroup(p)
            delta = 0 if prev_ticks is None else st.ticks - prev_ticks
            p.ticks_delta = delta
            p.cpu_seconds = delta / self.hz
            p.cpu = process_cpu_percent(prev_ticks, st.ticks, elapsed, self.ncpu, self.hz)
            # io: only readable for own processes; skip while the process
            # sleeps (no CPU time means no new I/O was issued)
            p.disk = 0.0
            p.disk_delta = 0
            if p.io_ok and (p.uid == self.my_uid) and (fresh or delta > 0):
                io = read_bytes(self._path(pid, "io"), True)
                if io is None:
                    p.io_ok = False
                else:
                    total = sum(parse_io(io))
                    if p.io_total is not None and now > p.io_time:
                        d = max(0, total - p.io_total)
                        p.disk_delta = d
                        p.disk = d / (now - p.io_time)
                    p.io_total = total
                    p.io_time = now
        self.removed = [p for pid, p in old.items()
                        if pid not in new or new[pid] is not p]
        self.added = added
        self.procs = new
        return new


def _count_cpus(root):
    text = read_text(root + "/stat")
    n = sum(1 for line in text.splitlines() if line.startswith("cpu") and line[3:4].isdigit())
    return n or None


def descendants(procs, pid):
    """pids of all descendants of pid (children first order reversed: leaves last)."""
    kids = {}
    for p in procs.values():
        kids.setdefault(p.ppid, []).append(p.pid)
    out = []
    todo = [pid]
    while todo:
        cur = todo.pop()
        for k in kids.get(cur, ()):
            if k not in out and k != pid:
                out.append(k)
                todo.append(k)
    return out


# ------------------------------------------------------------------ process actions

NICE_LEVELS = [("High", -10), ("Above normal", -5), ("Normal", 0),
               ("Below normal", 5), ("Low", 19)]


def priority_name(nice):
    if nice <= -10:
        return "High"
    if nice < 0:
        return "Above normal"
    if nice == 0:
        return "Normal"
    if nice < 19:
        return "Below normal"
    return "Low"


def nice_for(name):
    for n, v in NICE_LEVELS:
        if n == name:
            return v
    raise KeyError(name)


def read_start(pid, root="/proc"):
    data = read_bytes("%s/%d/stat" % (root, pid))
    if not data:
        return None
    try:
        return parse_stat(data).start
    except (ValueError, IndexError):
        return None


def same_process(pid, start, root="/proc"):
    """PID-reuse guard: is pid still the process that started at start?"""
    return start is not None and read_start(pid, root) == start


def _dry(entry, dry_run):
    if dry_run is None:
        dry_run = DRY_RUN
    if dry_run:
        ACTION_LOG.append(entry)
        import sys
        print("[dry-run]", " ".join(entry), file=sys.stderr)
    return dry_run


def send_signal(pid, start, sig, dry_run=None, root="/proc"):
    """Signal pid only if it is still the same process. Uses a pidfd so the
    check and the signal refer to the same process. Returns True if sent."""
    if pid <= 1 or pid == os.getpid():
        return False
    if not same_process(pid, start, root):
        return False
    if _dry(["kill", "-%d" % int(sig), str(pid)], dry_run):
        return True
    try:
        fd = os.pidfd_open(pid)
    except OSError:
        return False
    try:
        if not same_process(pid, start, root):
            return False
        signal.pidfd_send_signal(fd, sig)
        return True
    except OSError:
        return False
    finally:
        os.close(fd)


def alive(pid, start, root="/proc"):
    data = read_bytes("%s/%d/stat" % (root, pid))
    if not data:
        return False
    try:
        st = parse_stat(data)
    except (ValueError, IndexError):
        return False
    return st.start == start and st.state not in ("Z", "X")


def set_nice(pid, start, nice, dry_run=None, root="/proc"):
    """Lower the priority of an own process. Returns "ok", "gone" or
    "need-root" (raising priority or another user's process)."""
    if not same_process(pid, start, root):
        return "gone"
    try:
        cur = os.getpriority(os.PRIO_PROCESS, pid)
    except OSError:
        return "gone"
    if nice < cur:
        return "need-root"
    if _dry(["renice", str(nice), "-p", str(pid)], dry_run):
        return "ok"
    try:
        os.setpriority(os.PRIO_PROCESS, pid, nice)
        return "ok"
    except PermissionError:
        return "need-root"
    except OSError:
        return "gone"


# ------------------------------------------------------------------ swap and zram

def parse_swaps(text):
    """Rows of /proc/swaps: {"name", "type", "kind", "size", "used", "priority"}.
    Sizes in bytes; kind is "zram", "file" or "partition" (zram devices are
    listed as partitions by the kernel)."""
    out = []
    for line in (text or "").splitlines()[1:]:
        f = line.split()
        if len(f) < 4:
            continue
        try:
            size, used = int(f[2]) * 1024, int(f[3]) * 1024
            prio = int(f[4]) if len(f) > 4 else 0
        except ValueError:
            continue
        name = f[0].replace("\\040", " ")
        kind = "zram" if "/zram" in name else f[1]
        out.append({"name": name, "type": f[1], "kind": kind, "size": size, "used": used,
                    "priority": prio})
    return out


def swap_kind_name(kind):
    return {"zram": "Compressed RAM (zram)", "file": "File",
            "partition": "Partition"}.get(kind, kind)


def parse_vmstat(text):
    """{key: int} of /proc/vmstat."""
    out = {}
    for line in (text or "").splitlines():
        k, _, v = line.partition(" ")
        try:
            out[k] = int(v)
        except ValueError:
            continue
    return out


def swap_rates(prev, cur, elapsed, page_size=PAGE_SIZE):
    """(swap-in B/s, swap-out B/s) from two /proc/vmstat dicts (pswpin and
    pswpout count pages)."""
    if not prev or not cur or elapsed <= 0:
        return 0.0, 0.0
    rin = max(0, cur.get("pswpin", 0) - prev.get("pswpin", 0)) * page_size / elapsed
    rout = max(0, cur.get("pswpout", 0) - prev.get("pswpout", 0)) * page_size / elapsed
    return rin, rout


def parse_mm_stat(text):
    """/sys/block/zramN/mm_stat: {"orig", "compr", "mem_used", "mem_limit",
    "max_used", "same_pages", ...} in bytes (pages for the *_pages keys)."""
    keys = ("orig", "compr", "mem_used", "mem_limit", "max_used", "same_pages",
            "pages_compacted", "huge_pages", "huge_pages_since")
    vals = (text or "").split()
    out = {}
    for k, v in zip(keys, vals):
        try:
            out[k] = int(v)
        except ValueError:
            out[k] = 0
    return out


def parse_comp_algorithm(text):
    """The active algorithm of zramN/comp_algorithm ("lzo [zstd] lz4" -> zstd)."""
    m = re.search(r"\[([^\]]+)\]", text or "")
    if m:
        return m.group(1)
    parts = (text or "").split()
    return parts[0] if len(parts) == 1 else ""


def zram_devices(sys_root="/sys"):
    """Set-up zram devices: [{"name", "disksize", "algo", "orig", "compr",
    "mem_used", "ratio"}] (sizes in bytes). Devices with disksize 0 are unused."""
    out = []
    try:
        names = sorted(n for n in os.listdir(sys_root + "/block") if n.startswith("zram"))
    except OSError:
        return out
    for n in names:
        base = "%s/block/%s" % (sys_root, n)
        disksize = read_int(base + "/disksize", 0) or 0
        if not disksize:
            continue
        mm = parse_mm_stat(read_text(base + "/mm_stat"))
        orig, compr = mm.get("orig", 0), mm.get("compr", 0)
        out.append({"name": n, "disksize": disksize,
                    "algo": parse_comp_algorithm(read_text(base + "/comp_algorithm")),
                    "orig": orig, "compr": compr, "mem_used": mm.get("mem_used", 0),
                    "ratio": orig / compr if compr else 0.0})
    return out


# ------------------------------------------------------------------ system

def _block_devices(sys_root):
    out = []
    try:
        names = sorted(os.listdir(sys_root + "/block"))
    except OSError:
        return out
    for n in names:
        # zram is compressed RAM, not a disk: the Memory and Swap views show it
        if n.startswith(("loop", "ram", "dm-", "md", "sr", "fd", "zram")):
            continue
        out.append(n)
    return out


def disk_info(name, sys_root="/sys"):
    base = "%s/block/%s" % (sys_root, name)
    model = read_text(base + "/device/model").strip() or read_text(
        base + "/device/name").strip()
    rot = read_int(base + "/queue/rotational", 0)
    size = (read_int(base + "/size", 0) or 0) * 512
    removable = read_int(base + "/removable", 0)
    if name.startswith("nvme"):
        kind = "SSD (NVMe)"
    elif rot:
        kind = "HDD"
    else:
        kind = "SSD"
    if removable:
        kind += ", removable"
    return {"name": name, "model": model or name, "type": kind, "capacity": size}


def net_interfaces(sys_root="/sys"):
    """Physical network interfaces: [(name, "Wi-Fi"|"Ethernet")]."""
    out = []
    try:
        names = sorted(os.listdir(sys_root + "/class/net"))
    except OSError:
        return out
    for n in names:
        if n == "lo":
            continue
        base = "%s/class/net/%s" % (sys_root, n)
        if not os.path.exists(base + "/device"):
            continue  # virtual (bridge, veth, tun, docker)
        wifi = os.path.exists(base + "/wireless") or os.path.exists(base + "/phy80211")
        out.append((n, "Wi-Fi" if wifi else "Ethernet"))
    return out


def cache_sizes(sys_root="/sys"):
    """Total L1/L2/L3 bytes over unique cache instances."""
    seen = set()
    totals = {}
    base = sys_root + "/devices/system/cpu"
    try:
        cpus = [c for c in os.listdir(base) if re.fullmatch(r"cpu\d+", c)]
    except OSError:
        return totals
    for c in cpus:
        cdir = "%s/%s/cache" % (base, c)
        try:
            idxs = [i for i in os.listdir(cdir) if i.startswith("index")]
        except OSError:
            continue
        for i in idxs:
            d = "%s/%s" % (cdir, i)
            level = read_text(d + "/level").strip()
            typ = read_text(d + "/type").strip()
            shared = read_text(d + "/shared_cpu_list").strip() or c
            key = (level, typ, shared)
            if key in seen:
                continue
            seen.add(key)
            size = read_text(d + "/size").strip()
            m = re.match(r"(\d+)([KMG]?)", size)
            if not m:
                continue
            mult = {"": 1, "K": 1024, "M": 1024 ** 2, "G": 1024 ** 3}[m.group(2)]
            totals["L" + level] = totals.get("L" + level, 0) + int(m.group(1)) * mult
    return totals


def pci_name(vendor, device, ids_paths=("/usr/share/hwdata/pci.ids",
                                        "/usr/share/misc/pci.ids")):
    """Look a PCI vendor:device up in pci.ids (both hex strings, no 0x)."""
    vendor = vendor.lower()
    device = device.lower()
    for path in ids_paths:
        try:
            f = open(path, encoding="utf-8", errors="replace")
        except OSError:
            continue
        with f:
            vname = None
            for line in f:
                if line.startswith("#") or not line.strip():
                    continue
                if not line.startswith("\t"):
                    if vname is not None:
                        break
                    if line[:4].lower() == vendor:
                        vname = line[4:].strip()
                elif vname is not None and not line.startswith("\t\t"):
                    if line[1:5].lower() == device:
                        name = line[5:].strip()
                        m = re.search(r"\[(.+)\]", name)
                        short = vname.split("[")[0].replace(" Corporation", "").strip()
                        return "%s %s" % (short, m.group(1) if m else name)
            if vname:
                return vname
    return ""


class GPU:
    def __init__(self, card, sys_root="/sys"):
        self.card = card
        self.base = "%s/class/drm/%s/device" % (sys_root, card)
        uevent = read_text(self.base + "/uevent")
        self.driver = ""
        self.pci = ""
        for line in uevent.splitlines():
            if line.startswith("DRIVER="):
                self.driver = line[7:]
            elif line.startswith("PCI_SLOT_NAME="):
                self.pci = line[14:]
        self.busy_path = self.base + "/gpu_busy_percent"
        self.has_busy = os.path.exists(self.busy_path)
        self.vendor = read_text(self.base + "/vendor").strip().replace("0x", "")
        self.device = read_text(self.base + "/device").strip().replace("0x", "")
        self._name = None
        self.drm_base = "%s/class/drm/%s" % (sys_root, card)

    @property
    def name(self):
        if self._name is None:
            self._name = pci_name(self.vendor, self.device) or (
                "%s GPU" % self.driver if self.driver else self.card)
        return self._name

    def busy(self):
        return read_int(self.busy_path, 0) if self.has_busy else None

    def freq(self):
        cur = read_int(self.drm_base + "/gt_cur_freq_mhz")
        mx = read_int(self.drm_base + "/gt_max_freq_mhz")
        if cur is None:
            text = read_text(self.base + "/pp_dpm_sclk")
            for line in text.splitlines():
                if line.strip().endswith("*"):
                    m = re.search(r"(\d+)\s*Mhz", line, re.I)
                    if m:
                        cur = int(m.group(1))
        return cur, mx

    def vram(self):
        used = read_int(self.base + "/mem_info_vram_used")
        total = read_int(self.base + "/mem_info_vram_total")
        return used, total


def gpu_cards(sys_root="/sys"):
    try:
        names = sorted(os.listdir(sys_root + "/class/drm"))
    except OSError:
        return []
    out = []
    for n in names:
        if re.fullmatch(r"card\d+", n) and os.path.exists(
                "%s/class/drm/%s/device" % (sys_root, n)):
            out.append(GPU(n, sys_root))
    return out


def parse_drm_fdinfo(text):
    """(client id, pdev, {engine: ns}) from a DRM fdinfo, or None."""
    if "drm-driver:" not in text:
        return None
    cid = None
    pdev = ""
    engines = {}
    for line in text.splitlines():
        if line.startswith("drm-client-id:"):
            cid = line.split(":", 1)[1].strip()
        elif line.startswith("drm-pdev:"):
            pdev = line.split(":", 1)[1].strip()
        elif line.startswith("drm-engine-") and not line.startswith("drm-engine-capacity"):
            k, _, v = line.partition(":")
            try:
                engines[k[11:]] = int(v.split()[0])
            except (ValueError, IndexError):
                pass
    return cid, pdev, engines


def parse_net_sockets(text, udp=False):
    """{inode: connected?} from /proc/net/tcp[6] or udp[6]."""
    out = {}
    for line in text.splitlines()[1:]:
        p = line.split()
        if len(p) < 10:
            continue
        try:
            inode = int(p[9])
        except ValueError:
            continue
        if udp:
            remote = p[2].split(":")
            connected = remote[-1] != "0000" or set(remote[0]) != {"0"}
        else:
            connected = p[3] == "01"  # TCP_ESTABLISHED
        out[inode] = connected
    return out


class FdScanner:
    """Every few seconds, walk the fds of own processes (others are not
    readable without root). Gives the closest Linux equivalents of Windows
    per-process columns: open files ("handles"), active network connections
    (sockets matched against /proc/net/tcp|udp) and the DRM fds that the
    GPU sampler reads engine time from."""

    def __init__(self, root="/proc", interval=5.0):
        self.root = root
        self.interval = interval
        self.last = None
        self.drm = {}  # pid -> [fd names pointing at /dev/dri]
        self.scanned = {}  # pid -> start of the process the fds belong to

    def _walk(self, pid):
        d = "%s/%d/fd" % (self.root, pid)
        try:
            names = os.listdir(d)
        except OSError:
            return None, [], []
        socks = []
        dri = []
        for fd in names:
            try:
                link = os.readlink(d + "/" + fd)
            except OSError:
                continue
            if link.startswith("socket:["):
                try:
                    socks.append(int(link[8:-1]))
                except ValueError:
                    pass
            elif link.startswith("/dev/dri/"):
                dri.append(fd)
        return len(names), socks, dri

    def scan(self, procs, my_uid, now, force=False):
        """Returns True when a scan ran."""
        new_pids = [p for p in procs.values() if not p.kernel and p.uid == my_uid
                    and self.scanned.get(p.pid) != p.start]
        full = force or self.last is None or now - self.last >= self.interval
        if not full and not new_pids:
            return False
        targets = [p for p in procs.values() if not p.kernel and p.uid == my_uid] \
            if full else new_pids
        if full:
            self.last = now
        conn = {}
        for name, udp in (("tcp", False), ("tcp6", False), ("udp", True), ("udp6", True)):
            conn.update(parse_net_sockets(read_text("%s/net/%s" % (self.root, name)), udp))
        for p in targets:
            n, socks, dri = self._walk(p.pid)
            self.scanned[p.pid] = p.start
            p.nfds = n
            p.conns = sum(1 for i in socks if conn.get(i)) if n is not None else None
            if dri:
                self.drm[p.pid] = dri
            else:
                self.drm.pop(p.pid, None)
        if full:
            for pid in list(self.scanned):
                if pid not in procs:
                    del self.scanned[pid]
                    self.drm.pop(pid, None)
        return True


class DrmUsage:
    """GPU busy % from the DRM fdinfo engine counters of own processes
    (works for i915/xe/amdgpu without root)."""

    def __init__(self, root="/proc", scanner=None):
        self.root = root
        self.scanner = scanner or FdScanner(root)
        self.prev = {}  # (pdev, client) -> {engine: ns}
        self.prev_time = None
        self.client_of = {}  # (pid, fd) -> (pdev, client id)

    @property
    def fds(self):
        return self.scanner.drm

    def sample(self, procs, my_uid, now):
        """procs/my_uid kept for the API; the scanner owns the fd list."""
        cur = {}
        cache = self.client_of
        live = set()
        for pid, fds in self.fds.items():
            for fd in fds:
                key = (pid, fd)
                live.add(key)
                known = cache.get(key)
                # many fds of one process share a DRM client: read each once
                if known is not None and known in cur:
                    continue
                info = parse_drm_fdinfo(read_text("%s/%d/fdinfo/%s" % (self.root, pid, fd)))
                if info and info[0] is not None:
                    ck = (info[1], info[0])
                    cache[key] = ck
                    cur[ck] = info[2]
        for key in [k for k in cache if k not in live]:
            del cache[key]
        busy = {}  # pdev -> {engine: delta ns}
        if self.prev_time is not None and now > self.prev_time:
            for key, eng in cur.items():
                before = self.prev.get(key)
                if before is None:
                    continue
                for e, ns in eng.items():
                    d = ns - before.get(e, ns)
                    if d > 0:
                        busy.setdefault(key[0], {}).setdefault(e, 0)
                        busy[key[0]][e] += d
        elapsed_ns = (now - self.prev_time) * 1e9 if self.prev_time else 0
        self.prev = cur
        self.prev_time = now
        out = {}
        for pdev, eng in busy.items():
            out[pdev] = {e: min(100.0, 100.0 * ns / elapsed_ns) for e, ns in eng.items()} \
                if elapsed_ns else {}
        return out


class SystemSampler:
    """CPU, memory, disk, network and GPU totals with 60-sample histories."""

    HISTORY = 60

    def __init__(self, proc_root="/proc", sys_root="/sys", clock=time.monotonic):
        self.proc_root = proc_root
        self.sys_root = sys_root
        self.clock = clock
        self.last = None
        self.prev_cpu = None
        self.prev_cores = []
        self.cpu = 0.0
        self.cores = []
        self.cpu_hist = []
        self.core_hist = []
        self.mem = {}
        self.mem_hist = []
        self.disks = [disk_info(n, sys_root) for n in _block_devices(sys_root)]
        self.prev_disk = {}
        self.disk_now = {}  # name -> (active, rd, wr, resp)
        self.disk_hist = {d["name"]: [] for d in self.disks}
        self.disk_rw_hist = {d["name"]: ([], []) for d in self.disks}
        self.nets = net_interfaces(sys_root)
        self.prev_net = {}
        self.net_now = {}  # name -> (rx B/s, tx B/s)
        self.net_hist = {n: ([], []) for n, _ in self.nets}
        self.gpus = gpu_cards(sys_root)
        self.gpu_now = {}  # card -> busy %
        self.gpu_hist = {g.card: [] for g in self.gpus}
        self.gpu_engines = {}
        self.fdscan = FdScanner(proc_root)
        self.drm = DrmUsage(proc_root, self.fdscan)
        self.swaps = []  # parse_swaps rows
        self.swap_hist = []  # % of all swap in use
        self.swap_io_hist = ([], [])  # swap-in, swap-out B/s
        self.swap_now = (0.0, 0.0)
        self.prev_vmstat = None
        self.zram = []  # zram_devices rows
        self.nvidia = []  # parsed nvidia-smi rows (filled by the GUI)
        self.extra = {}
        cpuinfo = parse_cpuinfo(read_text(proc_root + "/cpuinfo"))
        self.cpu_info = cpuinfo
        self._caches = None

    @property
    def caches(self):
        if self._caches is None:
            self._caches = cache_sizes(self.sys_root)
        return self._caches

    def _push(self, hist, value):
        hist.append(value)
        if len(hist) > self.HISTORY:
            del hist[0]

    def sample(self, procs=None, my_uid=None):
        now = self.clock()
        elapsed = (now - self.last) if self.last is not None else 0.0
        self.last = now
        total, cores, extra = parse_cpu_stat(read_text(self.proc_root + "/stat"))
        self.extra = extra
        self.cpu = cpu_percent(self.prev_cpu, total) if self.prev_cpu else 0.0
        self.cores = [cpu_percent(self.prev_cores[i] if i < len(self.prev_cores) else None, c)
                      for i, c in enumerate(cores)]
        self.prev_cpu, self.prev_cores = total, cores
        self._push(self.cpu_hist, self.cpu)
        while len(self.core_hist) < len(self.cores):
            self.core_hist.append([])
        for i, v in enumerate(self.cores):
            self._push(self.core_hist[i], v)
        self.mem = parse_meminfo(read_text(self.proc_root + "/meminfo"))
        self._push(self.mem_hist, self.mem_used_fraction() * 100)
        stats = parse_diskstats(read_text(self.proc_root + "/diskstats"))
        for d in self.disks:
            n = d["name"]
            cur = stats.get(n)
            if cur is None:
                continue
            rates = disk_rates(self.prev_disk.get(n), cur, elapsed)
            self.prev_disk[n] = cur
            self.disk_now[n] = rates
            self._push(self.disk_hist[n], rates[0])
            self._push(self.disk_rw_hist[n][0], rates[1])
            self._push(self.disk_rw_hist[n][1], rates[2])
        net = parse_netdev(read_text(self.proc_root + "/net/dev"))
        for n, _kind in self.nets:
            cur = net.get(n)
            if cur is None:
                continue
            prev = self.prev_net.get(n)
            if prev and elapsed > 0:
                rx = max(0, cur[0] - prev[0]) / elapsed
                tx = max(0, cur[1] - prev[1]) / elapsed
            else:
                rx = tx = 0.0
            self.prev_net[n] = cur
            self.net_now[n] = (rx, tx)
            self._push(self.net_hist[n][0], rx)
            self._push(self.net_hist[n][1], tx)
        self.swaps = parse_swaps(read_text(self.proc_root + "/swaps"))
        tot = sum(d["size"] for d in self.swaps)
        self._push(self.swap_hist, 100.0 * sum(d["used"] for d in self.swaps) / tot
                   if tot else 0.0)
        vm = parse_vmstat(read_text(self.proc_root + "/vmstat"))
        self.swap_now = swap_rates(self.prev_vmstat, vm, elapsed)
        self.prev_vmstat = vm
        self._push(self.swap_io_hist[0], self.swap_now[0])
        self._push(self.swap_io_hist[1], self.swap_now[1])
        self.zram = zram_devices(self.sys_root)
        if procs:
            self.fdscan.scan(procs, my_uid, now)
        if self.gpus:
            need_drm = any(not g.has_busy for g in self.gpus)
            drm = self.drm.sample(procs or {}, my_uid, now) if need_drm and procs else {}
            for g in self.gpus:
                nv = self.nvidia_row(g.card)
                if g.has_busy:
                    v = float(g.busy() or 0)
                    self.gpu_engines[g.card] = {}
                elif nv is not None:
                    v = float(nv.get("util", 0))
                    self.gpu_engines[g.card] = {}
                else:
                    eng = drm.get(g.pci, {})
                    self.gpu_engines[g.card] = eng
                    v = max(eng.values()) if eng else 0.0
                self.gpu_now[g.card] = v
                self._push(self.gpu_hist[g.card], v)
        return self

    def nvidia_row(self, card):
        """nvidia-smi row for an NVIDIA DRM card (matched by order)."""
        nv = [g.card for g in self.gpus if g.driver == "nvidia"]
        if card not in nv or not self.nvidia:
            return None
        i = nv.index(card)
        return self.nvidia[i] if i < len(self.nvidia) else None

    # --- derived values
    def mem_used_fraction(self):
        tot = self.mem.get("MemTotal", 0)
        if not tot:
            return 0.0
        return (tot - self.mem.get("MemAvailable", self.mem.get("MemFree", 0))) / tot

    def mem_summary(self):
        m = self.mem
        tot = m.get("MemTotal", 0)
        avail = m.get("MemAvailable", m.get("MemFree", 0))
        cached = m.get("Cached", 0) + m.get("SReclaimable", 0) - m.get("Shmem", 0)
        free = m.get("MemFree", 0)
        return {"total": tot, "used": tot - avail, "available": avail,
                "cached": max(0, cached), "free": free,
                "committed": m.get("Committed_AS", 0), "commit_limit": m.get("CommitLimit", 0),
                "swap_total": m.get("SwapTotal", 0),
                "swap_used": m.get("SwapTotal", 0) - m.get("SwapFree", 0),
                "buffers": m.get("Buffers", 0), "shmem": m.get("Shmem", 0),
                "kernel": m.get("Slab", 0) + m.get("KernelStack", 0) + m.get("PageTables", 0)}

    def disk_total_active(self):
        return max((v[0] for v in self.disk_now.values()), default=0.0)

    def net_total(self):
        return sum(rx + tx for rx, tx in self.net_now.values())

    def cpu_speed(self):
        """Average current frequency in kHz."""
        base = self.sys_root + "/devices/system/cpu"
        vals = []
        for i in range(len(self.cores) or 1):
            v = read_int("%s/cpu%d/cpufreq/scaling_cur_freq" % (base, i))
            if v:
                vals.append(v)
        return sum(vals) / len(vals) if vals else 0

    def base_speed(self):
        base = self.sys_root + "/devices/system/cpu/cpu0/cpufreq/"
        return read_int(base + "base_frequency") or read_int(base + "cpuinfo_max_freq") or 0

    def uptime(self):
        text = read_text(self.proc_root + "/uptime")
        try:
            return float(text.split()[0])
        except (ValueError, IndexError):
            return 0.0


def format_uptime(seconds):
    s = int(seconds)
    d, s = divmod(s, 86400)
    h, s = divmod(s, 3600)
    m, s = divmod(s, 60)
    return "%d:%02d:%02d:%02d" % (d, h, m, s)


def iface_ipv4(name):
    """IPv4 address of an interface via SIOCGIFADDR (no traffic)."""
    import fcntl
    import socket
    import struct
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
            res = fcntl.ioctl(s.fileno(), 0x8915, struct.pack("256s", name[:15].encode()))
            return socket.inet_ntoa(res[20:24])
    except OSError:
        return ""


def iface_ipv6(name, proc_root="/proc"):
    """Addresses of an interface from /proc/net/if_inet6 (global first)."""
    out = []
    for line in read_text(proc_root + "/net/if_inet6").splitlines():
        p = line.split()
        if len(p) >= 6 and p[5] == name:
            h = p[0]
            addr = ":".join(h[i:i + 4] for i in range(0, 32, 4))
            import ipaddress
            a = str(ipaddress.IPv6Address(addr))
            scope = int(p[3], 16)
            out.append((scope, a))
    out.sort()
    return [a for _s, a in out]


NVIDIA_QUERY = ["nvidia-smi", "--query-gpu=index,name,utilization.gpu,memory.used,"
                "memory.total,clocks.gr", "--format=csv,noheader,nounits"]


def parse_nvidia_smi(text):
    """Rows of NVIDIA_QUERY output."""
    out = []
    for line in (text or "").splitlines():
        p = [x.strip() for x in line.split(",")]
        if len(p) < 6:
            continue

        def num(v):
            try:
                return float(v)
            except ValueError:
                return 0.0
        out.append({"index": int(num(p[0])), "name": p[1], "util": num(p[2]),
                    "mem_used": num(p[3]), "mem_total": num(p[4]), "clock": int(num(p[5]))})
    return out
