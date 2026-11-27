"""Harbor Settings > Privacy & security backend. Harbor runs it as
  python3 -I -c BRIDGE MODE [ARGS...]
and root work as  pkexec python3 -I -c BRIDGE root OP [ARGS...].
Modes print JSON (status reads) or plain log lines (actions).
Root ops are a fixed list; snap names and interfaces are validated.
Nothing is sent off this PC: virus scans use the local ClamAV only.
"""
import glob
import json
import os
import re
import shutil
import subprocess
import sys
import time

HOME = os.path.realpath(os.path.expanduser("~"))
STATE = os.path.join(HOME, ".local/state/harbor-settings")
QUAR = os.path.join(HOME, ".local/share/harbor-settings/quarantine")
CAM_CONF = "/etc/modprobe.d/harbor-no-camera.conf"
APP_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
IFACES = ("camera", "network")


def out(obj):
    print(json.dumps(obj), flush=True)


def sh(argv, timeout=60):
    try:
        r = subprocess.run(argv, capture_output=True, text=True, timeout=timeout)
        return r.returncode, r.stdout, r.stderr
    except (OSError, subprocess.TimeoutExpired) as e:
        return 127, "", str(e)


def have(cmd):
    return shutil.which(cmd) is not None


def installed(pkg):
    rc, so, _ = sh(["dpkg-query", "-W", "-f=${Status}", pkg])
    return rc == 0 and "install ok installed" in so


def active(unit):
    return sh(["systemctl", "is-active", unit])[1].strip() == "active"


def read(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            return f.read()
    except OSError:
        return ""


# ------------------------------------------------------------ overview

def tile(tid, title, state, level, why, action=""):
    return {"id": tid, "title": title, "state": state, "level": level,
            "why": why, "action": action}


def overview():
    t = []
    ufw = "ENABLED=yes" in read("/etc/ufw/ufw.conf")
    t.append(tile("firewall", "Firewall", "On" if ufw else "Off",
                  "safe" if ufw else "warning",
                  "" if ufw else "Other devices on the network can reach every open port on this PC.",
                  "tweaks-network"))
    au = read("/etc/apt/apt.conf.d/20auto-upgrades")
    auto = 'Unattended-Upgrade "1"' in au and installed("unattended-upgrades")
    t.append(tile("updates", "Automatic security updates", "On" if auto else "Off",
                  "safe" if auto else "critical",
                  "" if auto else "Security fixes are not installed until you update by hand.",
                  "updates"))
    rc, so, se = sh(["mokutil", "--sb-state"], 10)
    sb = "enabled" in (so + se).lower() and "disabled" not in (so + se).lower()
    t.append(tile("secureboot", "Secure Boot", "On" if sb else "Off",
                  "safe" if sb else "warning",
                  "" if sb else "Malware that loads before Linux (bootkits) is not blocked.", ""))
    tpm = os.path.isdir("/sys/class/tpm/tpm0")
    t.append(tile("tpm", "TPM security chip", "Present" if tpm else "Not found",
                  "safe" if tpm else "warning",
                  "" if tpm else "No hardware chip for disk-encryption keys and boot checks.", ""))
    rc, so, _ = sh(["lsblk", "-nro", "TYPE"], 10)
    crypt = "crypt" in so.split()
    t.append(tile("encryption", "Disk encryption", "On" if crypt else "Off",
                  "safe" if crypt else "warning",
                  "" if crypt else "Anyone who takes the laptop can read the disk. Turning it on needs a reinstall.",
                  ""))
    aa = read("/sys/module/apparmor/parameters/enabled").strip() == "Y"
    t.append(tile("apparmor", "AppArmor app confinement", "On" if aa else "Off",
                  "safe" if aa else "critical",
                  "" if aa else "Apps run without the profiles that limit what they may touch.", ""))
    cam = camera_whole()
    t.append(tile("camera", "Camera", cam["state"], "safe",
                  "", "security-camera"))
    clam = clam_status()
    if not clam["installed"]:
        t.append(tile("virus", "Virus scan", "ClamAV not installed", "warning",
                      "No local virus scanner. Install ClamAV to scan downloads.", "security-virus"))
    else:
        age = clam["db_age_days"]
        old = age is None or age > 7
        last = clam["last_scan"]
        state = f"Last scan {last['when']}: {last['found']} found" if last else "Never scanned"
        lvl = "critical" if last and last.get("found") else ("warning" if old else "safe")
        why = ("Threats were found; open Virus scan." if lvl == "critical"
               else "Virus definitions are more than a week old." if old else "")
        t.append(tile("virus", "Virus scan", state, lvl, why, "security-virus"))
    snitch = installed("opensnitch")
    t.append(tile("opensnitch", "App firewall (OpenSnitch)",
                  ("Running" if active("opensnitch") else "Installed, stopped") if snitch else "Not installed",
                  "safe", "" if snitch else "Optional: asks before any app connects to the internet.",
                  "security-network"))
    fw = firmware(quick=True)
    n = len(fw["updates"])
    t.append(tile("firmware", "Firmware", f"{n} update{'s' if n != 1 else ''} available" if n else "Up to date",
                  "warning" if n else "safe",
                  "Firmware updates often fix security holes in the BIOS or devices." if n else "",
                  "security-firmware"))
    out({"tiles": t})


# ------------------------------------------------------------ firmware

def firmware(quick=False):
    devices, updates = [], []
    if have("fwupdmgr"):
        if not quick:
            rc, so, _ = sh(["fwupdmgr", "get-devices", "--json"], 60)
            try:
                for d in json.loads(so).get("Devices", []):
                    if "updatable" not in d.get("Flags", []) and not d.get("Version"):
                        continue
                    devices.append({"name": d.get("Name", "?"), "vendor": d.get("Vendor", ""),
                                    "version": d.get("Version", ""),
                                    "updatable": "updatable" in d.get("Flags", [])})
            except ValueError:
                pass
        rc, so, _ = sh(["fwupdmgr", "get-updates", "--json", "--no-unreported-check"], 60)
        try:
            for d in json.loads(so).get("Devices", []):
                rel = (d.get("Releases") or [{}])[0]
                updates.append({"name": d.get("Name", "?"), "from": d.get("Version", ""),
                                "to": rel.get("Version", ""), "summary": rel.get("Summary", ""),
                                "id": d.get("DeviceId", "")})
        except ValueError:
            pass
    meta = glob.glob("/var/lib/fwupd/metadata/*/*.xml*")
    age = None
    if meta:
        age = int((time.time() - max(os.path.getmtime(m) for m in meta)) // 86400)
    return {"available": have("fwupdmgr"), "devices": devices, "updates": updates,
            "meta_age_days": age}


# ------------------------------------------------------------ apps

def flatpak_apps():
    if not have("flatpak"):
        return []
    rc, so, _ = sh(["flatpak", "list", "--app", "--columns=application,name"], 30)
    apps = []
    for ln in so.splitlines():
        parts = ln.split("\t")
        if len(parts) < 2 or not APP_RE.match(parts[0]):
            continue
        aid, name = parts[0], parts[1]
        rc, perm, _ = sh(["flatpak", "info", "--show-permissions", aid], 20)
        rc, meta, _ = sh(["flatpak", "info", "-M", aid], 20)
        def ctx(text, key):
            m = re.search(rf"^{key}=(.*)$", text, re.M)
            return [x for x in (m.group(1).split(";") if m else []) if x]
        portal = sh(["flatpak", "permission-show", aid], 20)[1]
        cam_portal = None
        for pl in portal.splitlines():
            f = pl.split("\t")
            if len(f) >= 4 and f[0] == "devices" and f[1] == "camera":
                cam_portal = f[3].strip() == "yes"
        dev = ctx(perm, "devices")
        apps.append({
            "kind": "flatpak", "id": aid, "name": name,
            "network": "network" in ctx(perm, "shared"),
            "network_possible": True,
            "network_default": "network" in ctx(meta, "shared"),
            "camera": "all" in dev and cam_portal is not False,
            "camera_possible": "all" in ctx(meta, "devices") or cam_portal is not None,
        })
    return apps


def snap_apps():
    if not have("snap"):
        return []
    rc, so, _ = sh(["snap", "connections", "--all"], 30)
    apps = {}
    for ln in so.splitlines()[1:]:
        f = ln.split()
        if len(f) < 3 or f[0] not in IFACES or ":" not in f[1]:
            continue
        snap = f[1].split(":", 1)[0]
        if not APP_RE.match(snap):
            continue
        a = apps.setdefault(snap, {"kind": "snap", "id": snap, "name": snap})
        a[f[0]] = f[2] != "-"
        a[f[0] + "_possible"] = True
    return sorted(apps.values(), key=lambda a: a["name"].lower())


def camera_whole():
    soft = sh(["gsettings", "get", "org.gnome.desktop.privacy", "disable-camera"], 10)[1].strip() == "true"
    driver_off = os.path.exists(CAM_CONF)
    loaded = os.path.isdir("/sys/module/uvcvideo")
    n = len(glob.glob("/sys/class/video4linux/video*"))
    state = ("Off (driver blocked)" if driver_off else "Off for apps" if soft else "On")
    return {"soft_off": soft, "driver_off": driver_off, "loaded": loaded,
            "devices": n, "state": state}


def network_whole():
    so = sh(["nmcli", "-t", "-f", "NETWORKING,WIFI", "general"], 10)[1].strip().split(":")
    return {"networking": so[0] == "enabled" if so else True,
            "wifi": len(so) > 1 and so[1] == "enabled"}


def apps(iface):
    rows = [a for a in flatpak_apps() + snap_apps() if a.get(iface + "_possible")]
    snitch = {"installed": installed("opensnitch"), "running": active("opensnitch"),
              "ui": have("opensnitch-ui")}
    out({"apps": rows, "camera": camera_whole(), "network": network_whole(), "opensnitch": snitch})


def flatpak_set(aid, iface, allow):
    if not APP_RE.match(aid) or iface not in IFACES:
        print("refused", flush=True)
        return 3
    if iface == "network":
        argv = ["flatpak", "override", "--user", "--share=network" if allow else "--unshare=network", aid]
        cmds = [argv]
    else:
        # Keep GPU access (dri) when cutting the other devices.
        cmds = [["flatpak", "override", "--user", "--device=all" if allow else "--nodevice=all"]
                + ([] if allow else ["--device=dri"]) + [aid],
                ["flatpak", "permission-set", "devices", "camera", aid, "yes" if allow else "no"]]
    rc_all = 0
    for argv in cmds:
        print("$ " + " ".join(argv), flush=True)
        rc, so, se = sh(argv, 30)
        if (so + se).strip():
            print((so + se).strip(), flush=True)
        rc_all |= rc
    if rc_all == 0:
        print("Restart the app for the change to take effect.", flush=True)
    return 1 if rc_all else 0


# ------------------------------------------------------------ ClamAV

def last_scan():
    try:
        with open(os.path.join(STATE, "last-scan.json")) as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def clam_status():
    dbs = glob.glob("/var/lib/clamav/*.c[vl]d")
    age = None
    if dbs:
        age = int((time.time() - max(os.path.getmtime(d) for d in dbs)) // 86400)
    return {"installed": have("clamscan"), "db": bool(dbs), "db_age_days": age,
            "freshclam": active("clamav-freshclam"), "last_scan": last_scan(),
            "quarantine": quarantine_list()}


def quarantine_list():
    try:
        with open(os.path.join(QUAR, "index.json")) as f:
            return json.load(f)
    except (OSError, ValueError):
        return []


def save_quarantine(items):
    os.makedirs(QUAR, mode=0o700, exist_ok=True)
    with open(os.path.join(QUAR, "index.json"), "w") as f:
        json.dump(items, f)


def scan(paths):
    paths = [os.path.realpath(os.path.expanduser(p)) for p in paths if p]
    paths = [p for p in paths if os.path.exists(p)]
    if not paths:
        print("Nothing to scan: the folder does not exist.", flush=True)
        return 2
    argv = ["clamscan", "-r", "-i", "--stdout", "--exclude-dir=" + re.escape(QUAR)] + paths
    print("$ " + " ".join(argv), flush=True)
    found = []
    try:
        p = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    except OSError as e:
        print(f"could not start clamscan: {e}", flush=True)
        return 127
    for ln in p.stdout:
        ln = ln.rstrip("\n")
        print(ln, flush=True)
        if ln.endswith(" FOUND") and ": " in ln:
            path, sig = ln[:-6].rsplit(": ", 1)
            found.append({"path": path, "sig": sig})
    rc = p.wait()
    os.makedirs(STATE, exist_ok=True)
    rec = {"when": time.strftime("%Y-%m-%d %H:%M"), "paths": paths,
           "found": len(found), "threats": found, "rc": rc}
    with open(os.path.join(STATE, "last-scan.json"), "w") as f:
        json.dump(rec, f)
    print(f"== {len(found)} threat{'s' if len(found) != 1 else ''} found", flush=True)
    return 0 if rc in (0, 1) else rc


def threat_paths(paths):
    """Only files the last scan reported may be quarantined or deleted."""
    known = {t["path"] for t in (last_scan() or {}).get("threats", [])}
    return [p for p in paths if p in known]


def quarantine(paths):
    items = quarantine_list()
    bad = 0
    for p in threat_paths(paths):
        dest = os.path.join(QUAR, f"{int(time.time()*1000)}-{os.path.basename(p)}.quarantined")
        try:
            os.makedirs(QUAR, mode=0o700, exist_ok=True)
            shutil.move(p, dest)
            os.chmod(dest, 0o000)
            items.append({"from": p, "file": dest, "when": time.strftime("%Y-%m-%d %H:%M")})
            print(f"quarantined {p}", flush=True)
        except OSError as e:
            print(f"could not move {p}: {e.strerror}", flush=True)
            bad += 1
    save_quarantine(items)
    forget(paths)
    return 1 if bad else 0


def delete_threats(paths):
    bad = 0
    for p in threat_paths(paths):
        try:
            os.unlink(p)
            print(f"deleted {p}", flush=True)
        except OSError as e:
            print(f"could not delete {p}: {e.strerror}", flush=True)
            bad += 1
    forget(paths)
    return 1 if bad else 0


def forget(paths):
    rec = last_scan()
    if rec:
        rec["threats"] = [t for t in rec["threats"] if t["path"] not in paths or os.path.exists(t["path"])]
        rec["found"] = len(rec["threats"])
        with open(os.path.join(STATE, "last-scan.json"), "w") as f:
            json.dump(rec, f)


def quarantine_action(files, restore):
    items = quarantine_list()
    keep, bad = [], 0
    for it in items:
        if it["file"] not in files:
            keep.append(it)
            continue
        if not os.path.realpath(it["file"]).startswith(QUAR + os.sep):
            keep.append(it)
            continue
        try:
            if restore:
                if os.path.exists(it["from"]):
                    raise OSError(17, "a file with that name exists again")
                os.chmod(it["file"], 0o600)
                shutil.move(it["file"], it["from"])
                print(f"restored {it['from']}", flush=True)
            else:
                os.chmod(it["file"], 0o600)
                os.unlink(it["file"])
                print(f"deleted {it['from']} for good", flush=True)
        except OSError as e:
            print(f"could not change {it['from']}: {e.strerror}", flush=True)
            keep.append(it)
            bad += 1
    save_quarantine(keep)
    return 1 if bad else 0


# ------------------------------------------------------------ root

def run_log(argv, env=None):
    print("$ " + " ".join(argv), flush=True)
    p = subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                       env=env)
    if p.stdout.strip():
        print(p.stdout.strip()[-4000:], flush=True)
    return p.returncode


def root(op, args):
    if os.geteuid() != 0:
        print("needs administrator rights", flush=True)
        return 2
    apt_env = dict(os.environ, DEBIAN_FRONTEND="noninteractive")
    if op == "camera-driver-off":
        with open(CAM_CONF, "w") as f:
            f.write("# Written by Harbor Settings > Camera access. Delete to allow the camera.\n"
                    "blacklist uvcvideo\n")
        rc = run_log(["modprobe", "-r", "uvcvideo"])
        if rc:
            print("The camera is in use; it stays blocked after the next restart.", flush=True)
        return 0
    if op == "camera-driver-on":
        if os.path.exists(CAM_CONF):
            os.unlink(CAM_CONF)
        return run_log(["modprobe", "uvcvideo"])
    if op in ("snap-connect", "snap-disconnect"):
        if len(args) != 2 or not APP_RE.match(args[0]) or args[1] not in IFACES:
            print("refused", flush=True)
            return 3
        verb = "connect" if op == "snap-connect" else "disconnect"
        return run_log(["snap", verb, f"{args[0]}:{args[1]}"])
    if op == "install-clamav":
        rc = run_log(["apt-get", "install", "-y", "clamav", "clamav-freshclam"], apt_env)
        return rc or run_log(["systemctl", "enable", "--now", "clamav-freshclam"])
    if op == "install-opensnitch":
        rc = run_log(["apt-get", "install", "-y", "opensnitch", "python3-opensnitch-ui"], apt_env)
        return rc or run_log(["systemctl", "enable", "--now", "opensnitch"])
    if op == "opensnitch-start":
        return run_log(["systemctl", "enable", "--now", "opensnitch"])
    if op == "opensnitch-stop":
        return run_log(["systemctl", "disable", "--now", "opensnitch"])
    if op == "clam-update":
        was = active("clamav-freshclam")
        if was:
            run_log(["systemctl", "stop", "clamav-freshclam"])
        rc = run_log(["freshclam", "--stdout"])
        if was:
            run_log(["systemctl", "start", "clamav-freshclam"])
        return rc
    print(f"refused: {op}", flush=True)
    return 3


def main():
    mode, args = sys.argv[1], sys.argv[2:]
    if mode == "overview":
        overview()
    elif mode == "firmware":
        out(firmware())
    elif mode == "fw-refresh":
        sys.exit(run_log(["fwupdmgr", "refresh", "--force"]))
    elif mode == "fw-update":
        sys.exit(run_log(["fwupdmgr", "update", "--assume-yes", "--no-reboot-check"]))
    elif mode == "apps":
        apps(args[0] if args else "camera")
    elif mode == "flatpak-set":
        sys.exit(flatpak_set(args[0], args[1], args[2] == "allow"))
    elif mode == "camera-soft":
        sys.exit(run_log(["gsettings", "set", "org.gnome.desktop.privacy", "disable-camera",
                          "false" if args[0] == "on" else "true"]))
    elif mode == "net-whole":
        what, on = args[0], args[1] == "on"
        argv = (["nmcli", "networking", "on" if on else "off"] if what == "networking"
                else ["nmcli", "radio", "wifi", "on" if on else "off"])
        sys.exit(run_log(argv))
    elif mode == "opensnitch-ui":
        subprocess.Popen(["opensnitch-ui"], start_new_session=True,
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    elif mode == "clam":
        out(clam_status())
    elif mode == "scan":
        sys.exit(scan(args))
    elif mode == "quarantine":
        sys.exit(quarantine(args))
    elif mode == "delete":
        sys.exit(delete_threats(args))
    elif mode in ("q-restore", "q-delete"):
        sys.exit(quarantine_action(set(args), mode == "q-restore"))
    elif mode == "root":
        sys.exit(root(args[0], args[1:]))
    else:
        sys.exit(2)


main()
