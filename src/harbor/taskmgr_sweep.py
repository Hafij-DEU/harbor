"""Memory Sweep for Harbor Task Manager (no GTK).

Ported from Memory Sweep 1.4.0: see what fills your RAM, then end it or
remove it. Process scan and grouping by app, extra helper copies of
Electron apps, files held in RAM (/tmp and /dev/shm), suggestions, and the
swap / zram / zswap plan that the root helper (data/harbor-swap) applies.

Safety rules (kept from Memory Sweep):
  * only own-uid processes are ever signalled; protected session processes
    (PROTECTED, PROTECTED_PREFIXES) and Task Manager itself with its
    ancestors are never ended;
  * every signal is preceded by a (pid, start time) check, and goes
    through taskmgr_proc.send_signal (pidfd) when the exact start is known;
  * files are removed only inside the tmpfs RAM folders (/tmp, /dev/shm),
    never their roots, never through a symlinked parent, symlinks are
    never followed (a link is removed as a link);
  * HARBOR_DRY_RUN=1 turns signals and removals into log entries.
Every function that reads the system takes its roots as parameters so the
tests can use temporary directories.
"""

import json
import os
import re
import shutil
import signal
import stat
import sys
import time
from collections import defaultdict

from . import taskmgr_proc as tp

PROC = "/proc"
UID = os.getuid()
CLK_TCK = tp.CLK_TCK
SELF_NAME = "Task Manager"
# tmpfs folders whose files may be removed. /run/user/<uid> also lives in RAM,
# but it holds the session's sockets (D-Bus, Wayland, PipeWire), so it is never offered.
FILE_ROOTS = ("/tmp", "/dev/shm")
REFRESH_SECONDS = 5
SMALL_GROUP = 10 << 20
SMALL_ITEM = 100 << 10
ACTION_LOG = tp.ACTION_LOG  # dry-run entries (shared with the rest of Task Manager)

NAMES = {
    "brave": "Brave", "chrome": "Google Chrome", "chromium": "Chromium", "firefox": "Firefox",
    "opera": "Opera", "claude-desktop": "Claude Desktop", "code": "VS Code",
    "gnome-shell": "GNOME Shell", "nautilus": "Files", "Xwayland": "Xwayland",
    "evolution-data-server": "Evolution data server", "gnome-software": "Software",
    "snapd": "snapd", "packagekitd": "PackageKit", "meshdrop-desktop": "MeshDrop",
}
# What a known app uses its extra copy for (see extra_copies).
EXTRA_NOTES = {
    "meshdrop-desktop": "MeshDrop's clipboard helper",
}
EXTRA_MIN = 20 << 20  # smaller extra copies are not suggested
HINTS = {
    "gradle-daemon": "Build daemon, starts again on the next build",
    "kotlin-daemon": "Build daemon, starts again on the next build",
    "claude-code": "Each instance is one Claude Code session",
    "brave": "Closing unused tabs also frees memory",
    "chrome": "Closing unused tabs also frees memory",
    "chromium": "Closing unused tabs also frees memory",
    "firefox": "Closing unused tabs also frees memory",
    "opera": "Closing unused tabs also frees memory",
}
# Processes the desktop session needs. Ending one logs you out or freezes the screen.
PROTECTED = {
    "systemd", "gnome-shell", "gnome-session-binary", "gnome-session-service", "gnome-session-ctl",
    "dbus-daemon", "dbus-broker", "dbus-broker-launch", "pipewire", "pipewire-pulse", "wireplumber",
    "Xwayland", "gdm-wayland-session", "gdm-x-session", "mutter-x11-frames", "at-spi-bus-launcher",
    "at-spi2-registryd", "xdg-desktop-portal", "xdg-desktop-portal-gnome", "xdg-desktop-portal-gtk",
    "xdg-permission-store", "xdg-document-portal", "gnome-keyring-daemon", "ibus-daemon", "gvfsd",
    "gnome-shell-calendar-server", "goa-daemon", "dconf-service", "polkit-gnome-authentication-agent-1",
    "mount.ntfs", "mount.ntfs-3g", "ntfs-3g", "sshfs", "rclone",
}
PROTECTED_PREFIXES = ("gsd-", "ibus-", "gvfs")
INTERPRETER = re.compile(r"(python|node|java|perl|ruby|gjs|bun|bash|sh|dash)[\d.]*")
UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")


def _read(path):
    try:
        with open(path) as f:
            return f.read()
    except (OSError, UnicodeDecodeError):
        return ""


def _dry(entry, dry_run):
    """True (and the entry logged) when this action must only be logged."""
    if dry_run is None:
        dry_run = tp.DRY_RUN
    if dry_run:
        ACTION_LOG.append(list(entry))
        print("[dry-run]", " ".join(entry), file=sys.stderr)
    return dry_run


def fmt_size(n):
    if n >= 1 << 30:
        return f"{n / (1 << 30):.1f} GB"
    if n >= 1 << 20:
        return f"{n / (1 << 20):.0f} MB"
    return f"{n / 1024:.0f} KB"


def fmt_age(seconds):
    seconds = max(0, int(seconds))
    if seconds < 60:
        return f"{seconds} s"
    if seconds < 3600:
        return f"{seconds // 60} min"
    if seconds < 86400:
        h, m = divmod(seconds // 60, 60)
        return f"{h} h {m} min" if m and h < 10 else f"{h} h"
    return f"{seconds // 86400} d"


# ---------------------------------------------------------------- system memory

def meminfo(proc=PROC):
    """/proc/meminfo in bytes (the same parser the system sampler uses)."""
    return tp.parse_meminfo(_read(f"{proc}/meminfo"))


def zram(proc=PROC, sys_root="/sys"):
    """[(device, stored_bytes, ram_bytes)] for each zram swap in use."""
    res = []
    for d in tp.parse_swaps(_read(f"{proc}/swaps")):
        if d["kind"] != "zram":
            continue
        mm = tp.parse_mm_stat(_read(f"{sys_root}/block/{os.path.basename(d['name'])}/mm_stat"))
        if len(mm) >= 3:
            res.append((d["name"], mm["orig"], mm["mem_used"]))
    return res


def ram_mounts(roots=FILE_ROOTS, proc=PROC):
    """[(mountpoint, used_bytes, total_bytes)] for the tmpfs folders in roots."""
    tmpfs = set()
    for line in _read(f"{proc}/mounts").splitlines():
        f = line.split()
        if len(f) >= 3 and f[2] == "tmpfs":
            tmpfs.add(f[1].replace("\\040", " "))
    res = []
    for mp in roots:
        if mp not in tmpfs:
            continue
        try:
            st = os.statvfs(mp)
        except OSError:
            continue
        res.append((mp, (st.f_blocks - st.f_bfree) * st.f_frsize, st.f_blocks * st.f_frsize))
    return res


def boot_time(proc=PROC):
    for line in _read(f"{proc}/stat").splitlines():
        if line.startswith("btime "):
            return int(line.split()[1])
    return 0


# ---------------------------------------------------------------- processes

class Proc:
    """start is the wall-clock start (seconds); ticks the exact start in clock
    ticks since boot (field 22 of stat) for the pidfd signal guard."""
    __slots__ = ("pid", "ppid", "uid", "comm", "exe", "argv", "start", "ram", "swap", "ticks")

    def __init__(self, pid, ppid, uid, comm, exe, argv, start, ram, swap, ticks=None):
        self.pid, self.ppid, self.uid, self.comm = pid, ppid, uid, comm
        self.exe, self.argv, self.start, self.ram, self.swap = exe, argv, start, ram, swap
        self.ticks = ticks

    @property
    def own(self):
        return self.uid == UID

    @property
    def mem(self):
        return self.ram + self.swap


def read_mem(pid, uid, proc=PROC):
    """(ram, swap) bytes of a process, or None when it is gone. RAM is PSS for
    own processes (shared pages split between their users, so a browser's 35
    processes add up to what it really costs), VmRSS for others."""
    base = f"{proc}/{pid}"
    status = {}
    text = _read(base + "/status")
    if not text:
        return None
    for line in text.splitlines():
        key, _, rest = line.partition(":")
        status[key] = rest.split()
    ram = int(status.get("VmRSS", ["0"])[0]) * 1024
    swap = int(status.get("VmSwap", ["0"])[0]) * 1024
    if uid == UID:
        roll = {}
        for line in _read(base + "/smaps_rollup").splitlines():
            key, _, rest = line.partition(":")
            if rest.split():
                roll[key] = int(rest.split()[0]) * 1024
        if "Pss" in roll:
            ram, swap = roll["Pss"], roll.get("SwapPss", swap)
    return ram, swap


def read_proc(pid, btime=None, proc=PROC):
    base = f"{proc}/{pid}"
    raw = _read(base + "/stat")
    if not raw:
        return None
    rp = raw.rfind(")")
    comm = raw[raw.find("(") + 1:rp]
    fields = raw[rp + 2:].split()
    ppid = int(fields[1])
    if pid == 2 or ppid == 2:
        return None  # kernel threads
    ticks = int(fields[19])
    start = (btime if btime is not None else boot_time(proc)) + ticks / CLK_TCK
    uid = -1
    for line in _read(base + "/status").splitlines():
        if line.startswith("Uid:"):
            uid = int(line.split()[1])
            break
    mem = read_mem(pid, uid, proc) or (0, 0)
    try:
        exe = os.readlink(base + "/exe").removesuffix(" (deleted)")
    except OSError:
        exe = ""
    argv = [a for a in _read(base + "/cmdline").split("\0") if a]
    return Proc(pid, ppid, uid, comm, exe, argv, start, mem[0], mem[1], ticks)


def scan_procs(proc=PROC):
    btime = boot_time(proc)
    procs = []
    for name in os.listdir(proc):
        if name.isdigit():
            try:
                p = read_proc(int(name), btime, proc)
            except (ValueError, IndexError):
                p = None
            if p:
                procs.append(p)
    return procs


def procs_from_sampler(sampled, btime, proc=PROC):
    """Sweep processes from Task Manager's ProcSampler readings: identity
    (pid, parent, uid, exe, argv, start) is reused, memory (PSS / swap) is
    read fresh because the sampler tracks private memory instead."""
    out = []
    for p in list(sampled.values()):
        if p.kernel:
            continue
        uid = -1 if p.uid is None else p.uid
        mem = read_mem(p.pid, uid, proc)
        if mem is None:
            continue  # ended since the last sample
        out.append(Proc(p.pid, p.ppid, uid, p.comm, (p.exe or "").removesuffix(" (deleted)"),
                        [a for a in p.argv if a], btime + p.start / CLK_TCK, mem[0], mem[1],
                        p.start))
    return out


def _stem(path):
    b = os.path.basename(path)
    for ext in (".py", ".js", ".mjs", ".cjs", ".jar", ".sh"):
        if b.endswith(ext):
            return b[:-len(ext)]
    return b


def _script(interp, args):
    """The script or program an interpreter runs, from its arguments."""
    skip = False
    for i, a in enumerate(args):
        if skip:
            skip = False
            continue
        if interp.startswith("python") and a == "-m" and i + 1 < len(args):
            return args[i + 1]
        if interp == "java" and a == "-jar" and i + 1 < len(args):
            return _stem(args[i + 1])
        if interp in ("bash", "sh", "dash") and a == "-c":
            return None
        if a in ("-cp", "-classpath", "--class-path", "-p", "--module-path", "-X", "-W", "-r",
                 "--require"):
            skip = True
            continue
        if a.startswith("-"):
            continue
        return a.rsplit(".", 1)[-1] if interp == "java" else _stem(a)
    return None


def label(p):
    """(group key, title) for a process."""
    cmd = " ".join(p.argv)
    if "GradleDaemon" in cmd:
        return "gradle-daemon", "Gradle daemon"
    if "KotlinCompileDaemon" in cmd:
        return "kotlin-daemon", "Kotlin daemon"
    argv = p.argv
    if len(argv) == 1 and " " in argv[0]:
        argv = argv[0].split()  # the process rewrote its title, like "npm exec pkg"
    # The exe link of other users' processes is unreadable; argv[0] beats the 15-char comm.
    base = os.path.basename(p.exe) if p.exe else os.path.basename(argv[0]) if argv else p.comm
    if base == "claude" and "claude-code" in p.exe:
        return "claude-code", "Claude Code sessions"
    if argv and os.path.basename(argv[0]) in ("npm", "npx"):
        return "npm-exec", "npm"
    m = INTERPRETER.fullmatch(base)
    if m:
        script = _script(m.group(1), argv[1:])
        if script:
            return f"{m.group(1)}:{script}", f"{script} ({m.group(1)})"
    return base, NAMES.get(base, base)


def is_protected(p):
    names = {p.comm, os.path.basename(p.exe)}
    return p.pid == 1 or bool(names & PROTECTED) or any(
        n.startswith(PROTECTED_PREFIXES) for n in names if n)


def self_and_ancestors(proc=PROC):
    pids, pid = set(), os.getpid()
    while pid > 1 and pid not in pids:
        pids.add(pid)
        p = read_proc(pid, proc=proc)
        if not p:
            break
        pid = p.ppid
    return pids


class Instance:
    """One running copy of an app: a root process and its same-app children."""

    def __init__(self, root, procs, tree):
        self.root, self.procs, self.tree = root, procs, tree

    @property
    def id(self):
        return (self.root.pid, self.root.start)

    @property
    def ram(self):
        return sum(p.ram for p in self.procs)

    @property
    def swap(self):
        return sum(p.swap for p in self.procs)

    @property
    def mem(self):
        return self.ram + self.swap


class Group:
    def __init__(self, key, title):
        self.key, self.title = key, title
        self.instances = []
        self.extras = []  # extra copies the app runs as helpers (see extra_copies)
        self.locked = None  # why it cannot be ended, or None

    @property
    def procs(self):
        return [p for i in self.instances for p in i.procs]

    @property
    def ram(self):
        return sum(i.ram for i in self.instances)

    @property
    def swap(self):
        return sum(i.swap for i in self.instances)

    @property
    def mem(self):
        return self.ram + self.swap

    @property
    def hint(self):
        return HINTS.get(self.key, "")

    @property
    def extra_note(self):
        return EXTRA_NOTES.get(self.key, f"A second copy of {self.title}")


def chromium_type(p):
    """The --type= of a Chromium or Electron helper process, or None for a main process."""
    argv = p.argv
    if len(argv) == 1 and " " in argv[0]:
        argv = argv[0].split()  # Chromium rewrites its process title into one string
    for a in argv[1:]:
        if a.startswith("--type="):
            return a[7:]
    return None


def extra_copies(inst):
    """Extra copies of a Chromium or Electron app inside one instance.

    Such an app runs one main process that starts helpers with --type= (renderer, GPU,
    network, zygote). Some apps also start a second full copy of themselves as a helper,
    MeshDrop for its clipboard. That copy brings its own GPU, network and zygote
    processes. It is a main process (no --type=, has --type= children) whose parent is
    a main process of the same program. The instance root is never one."""
    kids = defaultdict(list)
    for p in inst.tree:
        kids[p.ppid].append(p)

    def program(p):
        return p.exe or (p.argv[0] if p.argv else p.comm)

    def is_main(p):
        return chromium_type(p) is None and any(chromium_type(c) for c in kids[p.pid])

    def subtree(root):
        out, stack = [], [root]
        while stack:
            p = stack.pop()
            out.append(p)
            stack.extend(kids[p.pid])
        return out

    same_app = {p.pid for p in inst.procs}  # not the other apps a launcher started
    found, stack = [], [inst.root]
    while stack:
        p = stack.pop()
        for c in kids[p.pid]:
            if c.pid not in same_app:
                continue
            if is_main(p) and is_main(c) and program(c) == program(p):
                tree = subtree(c)  # a copy inside a copy is ended with it
                found.append(Instance(c, tree, tree))
            else:
                stack.append(c)
    found.sort(key=lambda i: -i.mem)
    return found


def build_groups(procs, keep=frozenset()):
    """Group processes by app. `keep` holds pids that must never be ended (this app)."""
    by_pid = {p.pid: p for p in procs}
    children = defaultdict(list)
    for p in procs:
        children[p.ppid].append(p)
    keys, groups = {}, {}
    titles = {}
    for p in procs:
        keys[p.pid], titles[p.pid] = label(p)
    # Wrappers like "npm exec pkg" or "sh -c cmd" only wait for the one program they
    # start; count them with that program. Repeat so npm -> sh -> node collapses fully.
    wrappers = {"npm-exec", "sh", "dash", "bash"}
    changed = True
    while changed:
        changed = False
        for p in procs:
            kids = children[p.pid]
            if keys[p.pid] in wrappers and len(kids) == 1 and keys[kids[0].pid] not in wrappers:
                keys[p.pid], titles[p.pid] = keys[kids[0].pid], titles[kids[0].pid]
                changed = True
    for p in procs:
        groups.setdefault(keys[p.pid], Group(keys[p.pid], titles[p.pid]))

    def walk(root, same_key):
        out, stack = [], [root]
        while stack:
            p = stack.pop()
            out.append(p)
            stack.extend(c for c in children[p.pid]
                         if not same_key or keys[c.pid] == keys[root.pid])
        return out

    for p in procs:
        if keys.get(p.ppid) != keys[p.pid] or p.ppid not in by_pid:
            groups[keys[p.pid]].instances.append(Instance(p, walk(p, True), walk(p, False)))
    for g in groups.values():
        g.instances.sort(key=lambda i: -i.mem)
        g.extras = [e for i in g.instances for e in extra_copies(i)]
        ps = g.procs
        if any(is_protected(p) for p in ps):
            g.locked = "Needed by your desktop session"
        elif not all(p.own for p in ps):
            g.locked = "Runs as another user"
        elif all(instance_locked(i, keep) for i in g.instances):
            g.locked = instance_locked(g.instances[0], keep)
    return sorted(groups.values(), key=lambda g: -g.mem)


def instance_locked(inst, keep=frozenset()):
    if not inst.root.own:
        return "Runs as another user"
    if is_protected(inst.root):
        return "Needed by your desktop session"
    if any(p.pid in keep for p in inst.tree):
        if any(p.pid == os.getpid() for p in inst.procs):
            return f"This is {SELF_NAME} itself"
        return f"{SELF_NAME} was started from this app"
    return None


def alive(pid, start, proc=PROC):
    """True if `pid` still runs the process that started at `start` (zombies count as gone)."""
    raw = _read(f"{proc}/{pid}/stat")
    if not raw:
        return False
    fields = raw[raw.rfind(")") + 2:].split()
    return fields[0] not in ("Z", "X") and \
        abs(boot_time(proc) + int(fields[19]) / CLK_TCK - start) < 1


def _send(pid, start, sig, ticks=None, dry_run=None):
    """Signal pid if it is still the process that started at start (pid reuse
    guard). With ticks the exact start is checked again through a pidfd."""
    if pid <= 1 or pid == os.getpid() or not alive(pid, start):
        return False
    if ticks is not None:
        return tp.send_signal(pid, ticks, sig, dry_run=dry_run)
    if _dry(["kill", "-%d" % int(sig), str(pid)], dry_run):
        return True
    try:
        os.kill(pid, sig)
        return True
    except OSError:
        return False


def stop(instances, force=False, keep=frozenset(), wait=4.0, dry_run=None):
    """End app instances. Polite: SIGTERM the root, then its leftover children.
    Force: SIGKILL the whole tree. Returns the (pid, start) pairs still running.
    In a dry run the signals are only logged and nothing is waited for."""
    targets = []
    for inst in instances:
        tree = [(p.pid, p.start, p.ticks) for p in inst.tree
                if p.own and p.pid not in keep and not is_protected(p)]
        root = (inst.root.pid, inst.root.start, inst.root.ticks)
        targets.append((root, tree))
    dry = tp.DRY_RUN if dry_run is None else dry_run
    sig = signal.SIGKILL if force else signal.SIGTERM
    for root, tree in targets:
        for pid, start, ticks in (tree if force else [t for t in tree if t == root]):
            _send(pid, start, sig, ticks, dry_run=dry)
    if dry:
        return []

    def wait_for(triples, seconds):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline and any(alive(t[0], t[1]) for t in triples):
            time.sleep(0.2)

    wait_for([r for r, tree in targets if r in tree], wait)
    if not force:
        rest = [t for _, tree in targets for t in tree if alive(t[0], t[1])]
        for pid, start, ticks in rest:
            _send(pid, start, signal.SIGTERM, ticks, dry_run=False)
        wait_for(rest, 1.5)
    return [(t[0], t[1]) for _, tree in targets for t in tree if alive(t[0], t[1])]


# ---------------------------------------------------------------- files in RAM

class Item:
    __slots__ = ("path", "size", "mtime", "is_dir", "own", "sockets", "note", "in_use")

    def __init__(self, path="", size=0):
        self.path, self.size, self.mtime, self.is_dir = path, size, 0, False
        self.own, self.sockets, self.note, self.in_use = True, 0, "", False

    @property
    def name(self):
        return os.path.basename(self.path)

    @property
    def unused(self):
        """Safe for "Remove all unused": own, not in use, no sockets inside."""
        return self.own and not self.in_use and not self.sockets


def measure(path):
    """(bytes on disk, newest mtime, sockets) without following symlinks."""
    try:
        st = os.lstat(path)
    except OSError:
        return 0, 0, 0
    size, newest = st.st_blocks * 512, st.st_mtime
    sockets = 1 if stat.S_ISSOCK(st.st_mode) else 0
    if not stat.S_ISDIR(st.st_mode):
        return size, newest, sockets
    stack = [path]
    while stack:
        try:
            it = os.scandir(stack.pop())
        except OSError:
            continue
        with it:
            for e in it:
                try:
                    s = e.stat(follow_symlinks=False)
                except OSError:
                    continue
                size += s.st_blocks * 512
                newest = max(newest, s.st_mtime)
                if stat.S_ISSOCK(s.st_mode):
                    sockets += 1
                elif stat.S_ISDIR(s.st_mode) and s.st_dev == st.st_dev:
                    stack.append(e.path)
    return size, newest, sockets


def describe(path, uid=UID):
    name = os.path.basename(path)
    claude = f"/tmp/claude-{uid}"
    if path == claude:
        return "Claude Code temp files"
    if path.startswith(claude + "/"):
        rel = path[len(claude) + 1:].split("/")
        if len(rel) == 2 and UUID.fullmatch(rel[1]):
            return "Claude Code session temp files"
        if len(rel) == 3 and rel[2] == "scratchpad":
            return "Claude Code session scratchpad"
        if len(rel) == 1 and rel[0].startswith("-"):
            return "Claude Code temp files for " + rel[0].replace("-", "/")
    if "compile-cache" in name:
        return "Compile cache, rebuilt when needed"
    if name.startswith("hsperfdata_"):
        return "Java performance data"
    if name.endswith(".log"):
        return "Log file"
    if name.startswith(("kotlin-daemon", "gradle")):
        return "Build tool temp files"
    if name.startswith((".X11-unix", ".ICE-unix", ".XIM-unix", ".font-unix")):
        return "Display server sockets"
    if name.startswith("systemd-private-"):
        return "Private temp folder of a system service"
    if name.startswith("snap-private-tmp"):
        return "Snap apps' private temp folders"
    return ""


def busy_markers(proc=PROC):
    """Working folders and argument words of your processes, to spot folders in use."""
    cwds, words = set(), set()
    try:
        names = os.listdir(proc)
    except OSError:
        names = []
    for name in names:
        if not name.isdigit():
            continue
        try:
            cwds.add(os.readlink(f"{proc}/{name}/cwd"))
        except OSError:
            continue
        for a in _read(f"{proc}/{name}/cmdline").split("\0"):
            words.update(UUID.findall(a))
    return cwds, words


def list_items(folder=None, markers=None, roots=None):
    """Items in `folder`, or the top level of every RAM folder, biggest first.
    roots: the RAM folders (default: the tmpfs ones of FILE_ROOTS)."""
    if folder:
        folders = [folder]
    else:
        folders = list(roots) if roots is not None else [mp for mp, _, _ in ram_mounts()]
    cwds, words = markers or busy_markers()
    items = []
    for d in folders:
        try:
            entries = list(os.scandir(d))
        except OSError:
            continue
        for e in entries:
            it = Item()
            it.path = e.path
            it.size, it.mtime, it.sockets = measure(e.path)
            try:
                st = e.stat(follow_symlinks=False)
                it.is_dir = stat.S_ISDIR(st.st_mode)
                it.own = st.st_uid == UID
            except OSError:
                it.is_dir, it.own = False, False
            it.note = describe(e.path)
            it.in_use = any(c == e.path or c.startswith(e.path + "/") for c in cwds) or \
                bool(UUID.fullmatch(it.name) and it.name in words)
            items.append(it)
    items.sort(key=lambda i: -i.size)
    return items


def hidden_usage(items, mounts):
    """{mountpoint: bytes} used in each RAM folder but not visible as files: deleted files
    still open in running apps (browsers keep shared memory like this) or unreadable folders."""
    out = {}
    for mp, used, _ in mounts:
        seen = sum(i.size for i in items if i.path.startswith(mp + "/"))
        if used - seen >= 20 << 20:
            out[mp] = used - seen
    return out


def removable(path, roots=None):
    """None if `path` may be removed, else the reason it may not."""
    parent = os.path.realpath(os.path.dirname(path))
    full = os.path.join(parent, os.path.basename(path))
    roots = list(roots) if roots is not None else [mp for mp, _, _ in ram_mounts()]
    if not os.path.basename(path) or not any(full.startswith(r.rstrip("/") + "/")
                                             for r in roots):
        return "Only files in " + " and ".join(FILE_ROOTS) + " can be removed"
    try:
        st = os.lstat(full)
    except OSError:
        return "It is already gone"
    if st.st_uid != UID:
        return "It belongs to another user"
    return None


def remove(path, roots=None, dry_run=None):
    """Delete a file or folder in a RAM folder. Returns the number of entries left behind.
    The resolved path (symlinked parents followed, the entry itself not) is removed."""
    why = removable(path, roots)
    if why:
        raise PermissionError(why)
    full = os.path.join(os.path.realpath(os.path.dirname(path)), os.path.basename(path))
    if _dry(["rm", "-rf", full], dry_run):
        return 0
    st = os.lstat(full)
    if not stat.S_ISDIR(st.st_mode):
        os.unlink(full)
        return 0
    failed = []
    shutil.rmtree(full, onexc=lambda fn, p, exc: failed.append(p))
    return len(failed)


def remove_unused(items, roots=None, dry_run=None):
    """Remove every unused item (Item.unused). Returns (removed, freed bytes,
    [(item, reason)] skipped)."""
    removed, freed, skipped = 0, 0, []
    for it in items:
        if not it.own:
            skipped.append((it, "not yours"))
            continue
        if it.in_use:
            skipped.append((it, "in use"))
            continue
        if it.sockets:
            skipped.append((it, "holds sockets"))
            continue
        try:
            left = remove(it.path, roots, dry_run)
        except OSError as e:
            skipped.append((it, getattr(e, "strerror", None) or str(e)))
            continue
        removed += 1
        if not left:
            freed += it.size
    return removed, freed, skipped


# ---------------------------------------------------------------- snapshot

class Snapshot:
    """Everything the Memory Sweep page shows. procs / mem may come from Task
    Manager's samplers (procs_from_sampler, SystemSampler.mem)."""

    def __init__(self, keep=frozenset(), procs=None, mem=None, proc=PROC, roots=FILE_ROOTS,
                 sys_root="/sys"):
        self.mem = mem if mem is not None else meminfo(proc)
        self.zram = zram(proc, sys_root)
        self.mounts = ram_mounts(roots, proc)
        self.swaps = swaps(proc)
        self.keep = keep
        self.groups = build_groups(procs if procs is not None else scan_procs(proc), keep)
        self.time = time.time()

    @property
    def total(self):
        return self.mem.get("MemTotal", 0)

    @property
    def available(self):
        return self.mem.get("MemAvailable", 0)

    @property
    def used(self):
        return self.total - self.available

    @property
    def swap_total(self):
        return self.mem.get("SwapTotal", 0)

    @property
    def swap_used(self):
        return self.swap_total - self.mem.get("SwapFree", 0)

    def swap_parts(self, zram):
        """(size, used) of the zram devices, or of the other swap devices."""
        parts = [s for s in self.swaps if ("/zram" in s[0]) == zram]
        return sum(s[2] for s in parts), sum(s[3] for s in parts)

    def group(self, key):
        return next((g for g in self.groups if g.key == key), None)

    def tips(self):
        """[(kind, text, page, group key or None)] suggestions, most useful first."""
        tips = []
        in_ram = sum(u for _, u, _ in self.mounts)
        if in_ram >= 300 << 20:
            where = " and ".join(mp for mp, u, _ in self.mounts if u >= 50 << 20) or "/tmp"
            tips.append(("files", f"{fmt_size(in_ram)} of files in {where} are held in RAM",
                         "files", None))
        for g in self.groups:
            extras = [e for e in g.extras if not instance_locked(e, self.keep)]
            mem = sum(e.mem for e in extras)
            if mem >= EXTRA_MIN:
                copy = "an extra copy" if len(extras) == 1 else f"{len(extras)} extra copies"
                tips.append(("extra", f"{g.title} runs {copy} of itself as a helper, using "
                             f"{fmt_size(mem)}", "apps", g.key))
        for g in self.groups:
            if g.key in ("gradle-daemon", "kotlin-daemon") and not g.locked:
                tips.append(("end", f"{g.title} uses {fmt_size(g.mem)} and starts again on "
                             "the next build", "apps", g.key))
        for g in self.groups:
            if g.key == "claude-code" and len(g.instances) >= 3:
                oldest = min(i.root.start for i in g.instances)
                tips.append(("apps", f"{len(g.instances)} Claude Code sessions use "
                             f"{fmt_size(g.mem)}; the oldest started "
                             f"{fmt_age(self.time - oldest)} ago", "apps", g.key))
        for g in self.groups[:3]:
            if g.key in HINTS and g.key not in ("gradle-daemon", "kotlin-daemon", "claude-code") \
                    and g.mem >= 1 << 30:
                tips.append(("apps", f"{g.title} uses {fmt_size(g.mem)} in {len(g.procs)} "
                             "processes", "apps", g.key))
        if not self.swap_parts(zram=True)[0] and self.total and self.used >= 0.75 * self.total:
            tips.append(("zram", "zram is off. Compressed swap in RAM gives you room before "
                         "apps slow down", "memory", None))
        return tips


# ---------------------------------------------------------------- swap, zram and zswap

SWAP_HELPER = "/usr/libexec/harbor/harbor-swap"
POLKIT_ACTION = "io.github.hafij_deu.Harbor.swap"
ZRAM_CONF = "/etc/default/zramswap"
ZRAMSWAP = "/usr/sbin/zramswap"
ZSWAP_PARAMS = "/sys/module/zswap/parameters"
ZRAM_DOCS = "https://docs.kernel.org/admin-guide/blockdev/zram.html"
ALGOS = ("lz4", "zstd", "lzo-rle")
ALGO_NAMES = ("lz4 (fastest)", "zstd (packs the most)", "lzo-rle (balanced)")
# What "Reset to defaults" puts in the form: half of RAM as zram (the zram-tools default
# of 256 MB is too small to matter), Ubuntu's 4 GB swap file and the kernel's vm values.
DEFAULTS = {"zram": 50, "algo": "zstd", "file": 4, "zswap": False, "swappiness": 60,
            "cache_pressure": 100}
# Form limits: (lowest, highest, step); the helper refuses anything outside them.
LIMITS = {"zram": (0, 150, 5), "file": (1, 64, 1), "swappiness": (0, 200, 5),
          "cache_pressure": (1, 500, 10)}
# Exit codes of the helper (data/harbor-swap).
SWAP_ERRORS = {
    2: "The helper refused these values",
    3: "Not enough free disk space for that swap file",
    4: "Not enough free RAM to take back what is in swap now. Close some apps first",
    5: "Swap files on btrfs need extra steps, so Task Manager leaves them alone",
    6: "zram-tools is not installed",
    7: "This kernel has no zswap",
    8: "apt-get failed. Check the internet connection and try again",
}


def swaps(proc=PROC):
    """[(device, kind, size_bytes, used_bytes)] from /proc/swaps."""
    return [(d["name"], d["type"], d["size"], d["used"])
            for d in tp.parse_swaps(_read(f"{proc}/swaps"))]


def _number(path, default):
    try:
        return int(_read(path).strip())
    except ValueError:
        return default


def zram_conf(path=ZRAM_CONF):
    """The active KEY=VALUE lines of the zram-tools config."""
    conf = {}
    for line in _read(path).splitlines():
        key, eq, value = line.strip().partition("=")
        if eq and key and not key.startswith("#"):
            conf[key] = value.strip().strip("\"'")
    return conf


def _fstab_swap_file(path="/etc/fstab"):
    for line in _read(path).splitlines():
        f = line.split()
        if len(f) >= 3 and f[2] == "swap" and f[0].startswith("/") and not f[0].startswith("/dev/"):
            return f[0]
    return None


def swap_status(proc=PROC, sys_root="/sys"):
    """What the Memory optimization settings show and start the form from."""
    total = meminfo(proc).get("MemTotal", 0)
    sw = swaps(proc)
    zr = [s for s in sw if "/zram" in s[0]]
    files = [s for s in sw if s[1] == "file"]
    conf = zram_conf()
    zsize = sum(s[2] for s in zr)
    try:
        percent = int(conf.get("PERCENT", ""))
    except ValueError:
        percent = round(zsize * 100 / total) if total and zsize else 0
    path = files[0][0] if files else _fstab_swap_file()
    size = files[0][2] if files else (os.path.getsize(path) if path and os.path.isfile(path)
                                      else 0)
    try:
        free = shutil.disk_usage(os.path.dirname(path or "/swap.img")).free
    except OSError:
        free = 0
    zswap = _read(f"{ZSWAP_PARAMS}/enabled").strip()
    stats = zram(proc, sys_root)
    return {
        "ram": total,
        "tools": os.path.isfile(ZRAM_CONF) and os.access(ZRAMSWAP, os.X_OK),
        "zram_on": bool(zr), "zram_percent": percent,
        "algo": conf.get("ALGO") if conf.get("ALGO") in ALGOS else "lz4",
        "zram_size": zsize, "zram_used": sum(s[3] for s in zr),
        "zram_stored": sum(s for _, s, _ in stats), "zram_ram": sum(r for _, _, r in stats),
        "file": path, "file_size": size, "file_used": files[0][3] if files else 0,
        "file_active": bool(files), "disk_free": free,
        "zswap": None if not zswap else zswap in ("Y", "1"),
        "zswap_compressor": _read(f"{ZSWAP_PARAMS}/compressor").strip(),
        "swappiness": _number(f"{proc}/sys/vm/swappiness", 60),
        "cache_pressure": _number(f"{proc}/sys/vm/vfs_cache_pressure", 100),
    }


def swap_form(st):
    """The form values that match the system as it is now."""
    return {
        "tools": None,  # None, "install", "reinstall" or "remove"
        "zram": st["zram_percent"] if st["zram_on"] else 0,
        "algo": st["algo"],
        "file": max(1, round(st["file_size"] / (1 << 30))) if st["file_size"] else 0,
        "zswap": bool(st["zswap"]),
        "swappiness": st["swappiness"],
        "cache_pressure": st["cache_pressure"],
    }


def plan_changes(st, form):
    """Helper arguments that turn the system `st` into what `form` asks for."""
    now = swap_form(st)
    tools = form.get("tools")
    args = [f"tools={tools}"] if tools else []
    has_tools = (st["tools"] or tools in ("install", "reinstall")) and tools != "remove"
    # A fresh zram-tools starts zram at its own size; set the asked one right after.
    if has_tools and (tools == "install" or (form["zram"], form["algo"]) != (now["zram"],
                                                                             now["algo"])):
        args.append(f"zram={form['zram']}:{form['algo']}")
    if form["file"] >= 1 and form["file"] != now["file"]:
        args.append(f"file={form['file']}")
    if st["zswap"] is not None and form["zswap"] != now["zswap"]:
        args.append(f"zswap={int(form['zswap'])}")
    for key in ("swappiness", "cache_pressure"):
        if form[key] != now[key]:
            args.append(f"{key}={form[key]}")
    return args


def describe_change(arg):
    """One helper argument in words."""
    key, _, value = arg.partition("=")
    if key == "tools":
        return {"install": "Install zram-tools", "reinstall": "Reinstall zram-tools",
                "remove": "Remove zram-tools and turn zram off"}[value]
    if key == "zram":
        percent, _, algo = value.partition(":")
        return "Turn zram off" if percent == "0" else f"zram at {percent}% of RAM, packed with {algo}"
    if key == "file":
        return f"Swap file of {value} GB"
    if key == "zswap":
        return "Turn zswap on" if value == "1" else "Turn zswap off"
    if key == "swappiness":
        return f"Swappiness {value}"
    return f"Cache pressure {value}"


def source_helper():
    """data/harbor-swap of a source checkout."""
    here = os.path.dirname(os.path.realpath(__file__))
    return os.path.realpath(os.path.join(here, "..", "..", "data", "harbor-swap"))


def swap_command(args):
    """pkexec command line for the helper. The installed copy has its own polkit
    action (POLKIT_ACTION); a source checkout runs data/harbor-swap through sh."""
    if os.access(SWAP_HELPER, os.X_OK):
        return ["pkexec", SWAP_HELPER, *args]
    return ["pkexec", "/bin/sh", source_helper(), *args]


def swap_result(code, out=""):
    """Toast text for the helper's exit code; out holds its "ok <key>" lines."""
    done = [w.split()[1] for w in (out or "").splitlines() if w.startswith("ok ") and
            len(w.split()) > 1]
    if code == 0:
        return "Changes applied"
    if code in (126, 127):  # pkexec: no password given
        return "Cancelled"
    msg = SWAP_ERRORS.get(code, f"The change failed (code {code})")
    if done:
        msg += f". Done before that: {', '.join(done)}"
    return msg


def usage_sample(proc=PROC):
    """(RAM total, RAM used, swap file or partition used, zram used) in bytes now."""
    m = meminfo(proc)
    total = m.get("MemTotal", 0)
    disk = comp = 0
    for dev, _, _, used in swaps(proc):
        if "/zram" in dev:
            comp += used
        else:
            disk += used
    return total, total - m.get("MemAvailable", 0), disk, comp


# ---------------------------------------------------------------- high-memory warning

NOTIFY_EVERY = 600  # at most one warning every 10 minutes


def should_notify(pct, enabled, threshold, last, now, every=NOTIFY_EVERY):
    return bool(enabled) and pct >= threshold and now - last >= every


def notification_text(pct, groups):
    """(title, body) of the high-memory warning."""
    body = f"{pct:.0f}% of RAM is in use."
    if groups:
        body += f" {groups[0].title} uses the most: {fmt_size(groups[0].mem)}."
    return "RAM is almost full", body


# ---------------------------------------------------------------- CLI

def report(as_json=False, out=None):
    """harbor-taskmgr sweep-report [--json] (the same as memory-sweep report)."""
    out = out or sys.stdout
    snap = Snapshot(self_and_ancestors())
    all_items = list_items()
    items = [i for i in all_items if i.size >= 10 << 20]
    if as_json:
        print(json.dumps({
            "ram": {"total": snap.total, "used": snap.used, "available": snap.available},
            "swap": {"total": snap.swap_total, "used": snap.swap_used},
            "zram": [{"device": d, "stored": s, "ram": r} for d, s, r in snap.zram],
            "ram_folders": [{"path": mp, "used": u, "size": t} for mp, u, t in snap.mounts],
            "apps": [{"key": g.key, "title": g.title, "ram": g.ram, "swap": g.swap,
                      "processes": len(g.procs), "instances": len(g.instances),
                      "locked": g.locked,
                      "extras": [{"pid": e.root.pid, "ram": e.ram, "swap": e.swap,
                                  "processes": len(e.tree), "note": g.extra_note,
                                  "locked": instance_locked(e, snap.keep)} for e in g.extras]}
                     for g in snap.groups if g.mem >= SMALL_GROUP],
            "files": [{"path": i.path, "size": i.size, "mine": i.own, "in_use": i.in_use,
                       "note": i.note} for i in items],
            "tips": [{"kind": k, "text": t, "page": pg, "key": key}
                     for k, t, pg, key in snap.tips()],
        }, indent=2), file=out)
        return 0
    w = lambda s="": print(s, file=out)  # noqa: E731
    w(f"RAM   {fmt_size(snap.used)} of {fmt_size(snap.total)} used, "
      f"{fmt_size(snap.available)} available")
    w(f"Swap  {fmt_size(snap.swap_used)} of {fmt_size(snap.swap_total)} used")
    for dev, stored, ram in snap.zram:
        w(f"zram  {dev}: {fmt_size(stored)} stored in {fmt_size(ram)} of RAM")
    for mp, used, total in snap.mounts:
        w(f"Files {mp}: {fmt_size(used)} held in RAM")
    w("\nApps")
    for g in snap.groups[:15]:
        n = len(g.instances)
        extra = f", {n} instances" if n > 1 else ""
        n = len(g.procs)
        w(f"  {fmt_size(g.mem):>9}  {g.title} ({n} process{'es' if n != 1 else ''}{extra})")
        for e in g.extras:
            w(f"  {fmt_size(e.mem):>9}    of it: extra copy, PID {e.root.pid} "
              f"({g.extra_note}); end it in Task Manager > Memory Sweep")
    for mp, n in hidden_usage(all_items, snap.mounts).items():
        w(f"      {mp}: {fmt_size(n)} of it is not visible as files (kept open by running apps)")
    if items:
        w("\nFiles in RAM")
        for i in items[:15]:
            w(f"  {fmt_size(i.size):>9}  {i.path}" + (f"  [{i.note}]" if i.note else ""))
    tips = snap.tips()
    if tips:
        w("\nSuggestions")
        for _, text, _, _ in tips:
            w("  - " + text)
    return 0
