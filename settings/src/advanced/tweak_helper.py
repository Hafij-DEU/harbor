#!/usr/bin/python3 -I
"""Glint root helper. Runs through pkexec: glint-helper run OP [OP...]

Every OP is checked with glint_core.valid_op and maps to commands defined in
glint_core, so the caller can choose operations but never commands.
"""
import glob
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.realpath(__file__))
sys.path.insert(0, HERE)
import glint_core as core  # noqa: E402

ENV = {
    "PATH": "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    "LANG": "C.UTF-8",
    "DEBIAN_FRONTEND": "noninteractive",
    "HOME": "/root",
}


def say(msg):
    print(msg, flush=True)


def sh(line):
    say(f"$ {line}")
    p = subprocess.run(["sh", "-c", line], env=ENV, stdout=sys.stdout, stderr=subprocess.STDOUT)
    return p.returncode == 0


def argv(cmd):
    say("$ " + " ".join(cmd))
    p = subprocess.run(cmd, env=ENV, stdout=sys.stdout, stderr=subprocess.STDOUT)
    return p.returncode == 0


def write_file(path, content):
    if not path.startswith(("/etc/sysctl.d/", "/etc/systemd/", "/etc/apt/apt.conf.d/",
                            "/etc/apt/preferences.d/", "/etc/NetworkManager/conf.d/",
                            "/etc/default/grub.d/")):
        say(f"refused path {path}")
        return False
    os.makedirs(os.path.dirname(path), mode=0o755, exist_ok=True)
    tmp = path + ".glint-tmp"
    with open(tmp, "w") as f:
        f.write(content)
    os.chmod(tmp, 0o644)
    os.replace(tmp, path)
    say(f"wrote {path}")
    return True


def remove_file(path):
    try:
        os.unlink(path)
        say(f"removed {path}")
    except FileNotFoundError:
        pass
    return True


def run_lines(lines):
    ok = True
    for ln in lines:
        if isinstance(ln, tuple):
            if ln[0] == "write":
                ok &= write_file(ln[1], ln[2])
            elif ln[0] == "remove":
                ok &= remove_file(ln[1])
            elif ln[0] == "crash":
                for f in glob.glob("/var/crash/*"):
                    if os.path.isfile(f) and not os.path.islink(f):
                        os.unlink(f)
                        say(f"removed {f}")
            elif ln[0] == "snaps":
                p = subprocess.run(["snap", "list", "--all"], env=ENV, capture_output=True, text=True)
                found = core.parse_snap_disabled(p.stdout)
                if not found:
                    say("no old snap revisions")
                for name, rev in found:
                    if re.fullmatch(r"[a-z0-9\-]+", name) and rev.isdigit():
                        ok &= argv(["snap", "remove", name, f"--revision={rev}"])
            elif ln[0] == "oldlogs":
                files = core.old_log_files()
                for f in files:
                    try:
                        os.unlink(f)
                    except OSError as e:
                        say(f"skipped {f}: {e.strerror}")
                say(f"removed {len(files)} old log files")
            elif ln[0] == "coredumps":
                n = 0
                for f in glob.glob("/var/lib/systemd/coredump/*"):
                    if os.path.isfile(f) and not os.path.islink(f):
                        os.unlink(f)
                        n += 1
                say(f"removed {n} core dumps")
            elif ln[0] == "rc":
                pkgs = [p for p in core.rc_packages() if core.PKG_RE.match(p.split(":")[0])]
                if not pkgs:
                    say("no leftover package settings")
                else:
                    ok &= argv(["dpkg", "--purge"] + pkgs)
            elif ln[0] in ("kernels", "locales"):
                if ln[0] == "kernels":
                    pkgs = core.old_kernel_pkgs(core.installed_pkgs(core.KERNEL_GLOBS), os.uname().release)
                else:
                    langs = core.used_langs()
                    say("languages kept: " + ", ".join(sorted(langs)))
                    pkgs = core.locale_pkgs_to_remove(core.installed_pkgs(core.LOCALE_GLOBS), langs)
                if not pkgs:
                    say("nothing to remove")
                elif removal_safe("purge", pkgs):
                    ok &= argv(["apt-get", "purge", "-y"] + pkgs)
                else:
                    ok = False
            elif ln[0] == "smart":
                disks = sorted(glob.glob("/dev/nvme[0-9]n[0-9]") + glob.glob("/dev/sd[a-z]"))
                for d in disks:
                    argv(["smartctl", "-H", d])
        else:
            ok &= sh(ln)
    return ok


def removal_safe(cmd, pkgs, allow=()):
    p = subprocess.run(["apt-get", "-s", cmd] + pkgs, env=ENV, capture_output=True, text=True)
    if p.returncode != 0:
        say("apt simulation failed:\n" + (p.stdout + p.stderr).strip())
        return False
    gone = set(core.parse_autoremove(p.stdout))
    running = os.uname().release
    bad = sorted(g for g in gone if g not in allow and core.is_protected(g.split(":")[0], running))
    if bad:
        say("refused: this would also remove " + ", ".join(bad))
        return False
    return True


def has_candidate(pkg):
    p = subprocess.run(["apt-cache", "policy", pkg], env=ENV, capture_output=True, text=True)
    m = re.search(r"Candidate:\s*(\S+)", p.stdout)
    return bool(m and m.group(1) != "(none)")


def snap_purge():
    """Remove every snap, purge snapd, block it from coming back."""
    ok = True
    if shutil.which("snap"):
        for _ in range(4):
            p = subprocess.run(["snap", "list"], env=ENV, capture_output=True, text=True)
            names = [n for n in core.snap_removal_order(p.stdout) if n != "snapd"]
            if p.returncode != 0 or not names:
                break
            for n in names:
                if re.fullmatch(r"[a-z0-9][a-z0-9\-]*", n):
                    argv(["snap", "remove", "--purge", n])
        p = subprocess.run(["snap", "list"], env=ENV, capture_output=True, text=True)
        left = [n for n in core.snap_removal_order(p.stdout) if n != "snapd"]
        if p.returncode == 0 and left:
            say("could not remove: " + ", ".join(left))
            return False
    if core.installed_apt(["snapd"]):
        if not removal_safe("purge", ["snapd"], allow={"snapd"}):
            return False
        ok &= argv(["apt-get", "purge", "-y", "snapd"])
    if ok:
        ok &= write_file(core.NO_SNAP_PIN, core.NO_SNAP_PIN_TEXT)
        with open("/proc/self/mounts") as f:
            mounts = [l.split()[1] for l in f if len(l.split()) > 1]
        for d in ("/snap", "/var/snap", "/var/lib/snapd", "/var/cache/snapd"):
            busy = any(m == d or m.startswith(d + "/") for m in mounts)
            if busy:
                say(f"kept {d}: something is still mounted there")
            elif os.path.isdir(d) and not os.path.islink(d):
                shutil.rmtree(d, ignore_errors=True)
                say(f"removed {d}")
        want = [p for p in ("gnome-software", "gnome-software-plugin-flatpak")
                if not core.installed_apt([p]) and has_candidate(p)]
        if want:
            say("installing GNOME Software with Flatpak support as the app store")
            ok &= argv(["apt-get", "install", "-y"] + want)
    return ok


def do_op(op):
    kind, _, arg = op.partition(":")
    if kind in ("tweak-apply", "tweak-revert"):
        t = core.TWEAK_BY_ID[arg]
        return run_lines(core.root_tweak_script(t, kind == "tweak-apply"))
    if kind == "clean":
        return run_lines(core.ROOT_CLEAN[arg])
    if kind == "util":
        return run_lines(core.UTIL_BY_ID[arg].script)
    if kind == "apt-install":
        return argv(["apt-get", "install", "-y", arg])
    if kind == "apt-remove":
        ok, gone, _ = core.apt_sim(["remove", arg])
        say("removes: " + ", ".join(gone))
        return removal_safe("remove", [arg]) and argv(["apt-get", "remove", "-y", arg])
    if kind == "snap-install":
        extra = ["--classic"] if arg in ("code",) else []
        return argv(["snap", "install", arg] + extra)
    if kind == "snap-remove":
        return argv(["snap", "remove", arg])
    if kind in ("bloat-remove", "bloat-purge"):
        b = core.BLOAT_BY_ID[arg]
        ok = True
        pkgs = sorted(core.installed_apt(b.apt))
        if pkgs:
            purge = kind == "bloat-purge"
            verb = "purge" if purge else "remove"
            try:
                direct, orphans = core.removal_plan(pkgs, purge)
            except RuntimeError as e:
                say(f"apt simulation failed: {e}")
                return False
            say("removes: " + ", ".join(direct))
            if orphans:
                say("no longer needed after this: " + ", ".join(orphans))
            # Only packages this removal orphans; never a system-wide autoremove.
            if not removal_safe(verb, pkgs + orphans):
                return False
            ok &= argv(["apt-get", verb, "-y"] + pkgs + orphans)
        snaps = core.installed_snaps()
        for s in b.snap:
            if s in snaps:
                ok &= argv(["snap", "remove"] + (["--purge"] if kind == "bloat-purge" else []) + [s])
        return ok
    if kind == "bloat-restore":
        b = core.BLOAT_BY_ID[arg]
        ok = True
        pkgs = [p for p in b.apt if has_candidate(p)]
        if b.snap and core.have("snap"):
            for s in b.snap:
                ok &= argv(["snap", "install", s])
            pkgs = [p for p in pkgs if p not in b.snap]
        if pkgs:
            ok &= argv(["apt-get", "install", "-y"] + pkgs)
        return ok
    if kind == "snap-purge":
        return snap_purge()
    if kind == "snap-restore":
        remove_file(core.NO_SNAP_PIN)
        return argv(["apt-get", "update"]) and argv(["apt-get", "install", "-y", "snapd"])
    if kind == "snapshot":
        if not core.have("timeshift"):
            say("Timeshift is not installed")
            return False
        say("Creating a Timeshift snapshot. The first one copies the whole system "
            "and can take 10-30 minutes; later ones only copy changes.")
        return argv(["timeshift", "--create", "--scripted", "--comments", "Glint: before changes"])
    return False


def main():
    if os.geteuid() != 0:
        say("glint-helper must run as root (through pkexec)")
        return 2
    if len(sys.argv) < 3 or sys.argv[1] != "run":
        say("usage: glint-helper run OP [OP...]")
        return 2
    ops = sys.argv[2:]
    bad = [o for o in ops if not core.valid_op(o)]
    if bad:
        say("refused unknown operation: " + " ".join(bad))
        return 3
    failed = 0
    for op in ops:
        say(f"== {op}")
        try:
            ok = do_op(op)
        except Exception as e:  # keep going with the other operations
            say(f"error: {e}")
            ok = False
        say(f"== {'ok' if ok else 'FAILED'} {op}")
        failed += not ok
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
