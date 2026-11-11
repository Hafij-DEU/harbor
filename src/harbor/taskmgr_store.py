"""Persistence and parsing helpers for Harbor Task Manager (no GTK).

Settings, app history, autostart entries and systemctl output. Every
function takes explicit paths so the tests can use temporary directories.
"""

import json
import os
import time

# ------------------------------------------------------------------ json files


def load_json(path, default):
    try:
        with open(path, encoding="utf-8") as f:
            data = json.load(f)
        return data if isinstance(data, type(default)) else default
    except (OSError, ValueError):
        return default


def save_json(path, data):
    """Atomic write (tmp file + rename)."""
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=1, sort_keys=True)
    os.replace(tmp, path)


# ------------------------------------------------------------------ settings

INTERVALS = [("High", 500), ("Normal", 1000), ("Low", 4000), ("Paused", 0)]
DEFAULTS = {
    "interval": "Normal",
    "start_page": "processes",
    "full_name": False,
    "group_by_type": True,
    "hidden_columns": [],
    "cpu_graph": "overall",
    "nav_collapsed": False,
    # high-memory warning (as in Memory Sweep: on, at 90 %, at most every 10 min)
    "notify_memory": True,
    "notify_at": 90,
}


class Settings:
    def __init__(self, path):
        self.path = path
        self.data = dict(DEFAULTS)
        self.data.update(load_json(path, {}))

    def __getitem__(self, key):
        return self.data.get(key, DEFAULTS.get(key))

    def __setitem__(self, key, value):
        self.data[key] = value
        self.save()

    def save(self):
        try:
            save_json(self.path, self.data)
        except OSError:
            pass

    def interval_ms(self):
        for name, ms in INTERVALS:
            if name == self["interval"]:
                return ms
        return 1000


# ------------------------------------------------------------------ app history


class AppHistory:
    """Cumulative CPU seconds and disk bytes per app since tracking began."""

    def __init__(self, path, clock=time.time):
        self.path = path
        self.clock = clock
        data = load_json(path, {})
        self.since = data.get("since") or clock()
        self.apps = data.get("apps") if isinstance(data.get("apps"), dict) else {}
        self.dirty = False

    def add(self, app_id, name, cpu_seconds, disk_bytes, icon=""):
        if cpu_seconds <= 0 and disk_bytes <= 0 and app_id in self.apps:
            return
        e = self.apps.setdefault(app_id, {"name": name, "cpu": 0.0, "disk": 0, "icon": icon})
        e["name"] = name or e.get("name") or app_id
        if icon:
            e["icon"] = icon
        e["cpu"] = e.get("cpu", 0.0) + max(0.0, cpu_seconds)
        e["disk"] = e.get("disk", 0) + max(0, int(disk_bytes))
        self.dirty = True

    def save(self):
        save_json(self.path, {"since": self.since, "apps": self.apps})
        self.dirty = False

    def clear(self):
        self.apps = {}
        self.since = self.clock()
        self.dirty = True
        self.save()


def format_cpu_time(seconds):
    s = int(seconds)
    h, s = divmod(s, 3600)
    m, s = divmod(s, 60)
    return "%d:%02d:%02d" % (h, m, s)


# ------------------------------------------------------------------ autostart

def parse_desktop(text):
    """[Desktop Entry] keys of a .desktop file (unlocalised + locale keys)."""
    out = {}
    group = None
    for line in text.splitlines():
        s = line.strip()
        if not s or s.startswith("#"):
            continue
        if s.startswith("[") and s.endswith("]"):
            group = s[1:-1]
            continue
        if group != "Desktop Entry" or "=" not in s:
            continue
        k, _, v = s.partition("=")
        out.setdefault(k.strip(), v.strip())
    return out


def _bool(v, default=False):
    if v is None:
        return default
    return v.strip().lower() == "true"


def shown_in(entry, desktops):
    only = [d for d in entry.get("OnlyShowIn", "").split(";") if d]
    nots = [d for d in entry.get("NotShowIn", "").split(";") if d]
    if only and not any(d in only for d in desktops):
        return False
    if any(d in nots for d in desktops):
        return False
    return True


def publisher_of(entry, file_id):
    for key in ("X-GNOME-Bugzilla-Bugzilla", "X-Publisher", "X-AppImage-Vendor"):
        if entry.get(key):
            return entry[key]
    base = file_id[:-8] if file_id.endswith(".desktop") else file_id
    parts = base.split(".")
    if len(parts) >= 3 and parts[0] in ("org", "com", "io", "net", "de", "dev", "app"):
        return ".".join(reversed(parts[:2]))
    return "—"


def list_autostart(user_dir, system_dirs, desktops=("GNOME",)):
    """Startup entries; user files override system ones with the same id."""
    files = {}
    for d in list(system_dirs)[::-1] + [user_dir]:
        try:
            names = os.listdir(d)
        except OSError:
            continue
        for n in names:
            if n.endswith(".desktop"):
                files.setdefault(n, {})["user" if d == user_dir else "system"] = \
                    os.path.join(d, n)
    out = []
    for fid, paths in sorted(files.items()):
        path = paths.get("user") or paths.get("system")
        try:
            with open(path, encoding="utf-8", errors="replace") as f:
                entry = parse_desktop(f.read())
        except OSError:
            continue
        if entry.get("Type", "Application") != "Application":
            continue
        hidden = _bool(entry.get("Hidden"))
        if hidden and "user" not in paths:
            continue  # deleted by the system itself
        if not shown_in(entry, desktops):
            continue
        enabled = _bool(entry.get("X-GNOME-Autostart-enabled"), True) and not hidden
        out.append({
            "id": fid, "name": entry.get("Name") or fid[:-8], "icon": entry.get("Icon", ""),
            "comment": entry.get("Comment", ""), "exec": entry.get("Exec", ""),
            "publisher": publisher_of(entry, fid), "enabled": enabled, "path": path,
            "user_path": paths.get("user"), "system_path": paths.get("system"),
        })
    return out


def _set_key(text, key, value):
    """Set key=value in [Desktop Entry], keeping every other line."""
    lines = text.splitlines()
    group = None
    start = end = None
    for i, line in enumerate(lines):
        s = line.strip()
        if s.startswith("[") and s.endswith("]"):
            if group == "Desktop Entry" and end is None:
                end = i
            group = s[1:-1]
            if group == "Desktop Entry":
                start = i
            continue
        if group == "Desktop Entry" and "=" in s and s.partition("=")[0].strip() == key:
            lines[i] = "%s=%s" % (key, value)
            return "\n".join(lines) + "\n"
    if start is None:
        lines = ["[Desktop Entry]", "%s=%s" % (key, value)] + lines
    else:
        if end is None:
            end = len(lines)
        while end > start + 1 and not lines[end - 1].strip():
            end -= 1
        lines.insert(end, "%s=%s" % (key, value))
    return "\n".join(lines) + "\n"


def set_autostart_enabled(entry, enabled, user_dir):
    """Write a user copy with X-GNOME-Autostart-enabled. Never touches the
    system file. Returns the path written."""
    src = entry.get("user_path") or entry.get("system_path") or entry["path"]
    with open(src, encoding="utf-8", errors="replace") as f:
        text = f.read()
    text = _set_key(text, "X-GNOME-Autostart-enabled", "true" if enabled else "false")
    if enabled and _bool(parse_desktop(text).get("Hidden")):
        text = _set_key(text, "Hidden", "false")
    os.makedirs(user_dir, exist_ok=True)
    dest = os.path.join(user_dir, entry["id"])
    tmp = dest + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(text)
    os.replace(tmp, dest)
    return dest


# ------------------------------------------------------------------ systemd services

def parse_units_json(text, scope="system"):
    """Rows of `systemctl list-units --type=service --all -o json`."""
    try:
        data = json.loads(text or "[]")
    except ValueError:
        return []
    out = []
    for u in data if isinstance(data, list) else []:
        name = u.get("unit", "")
        if not name.endswith(".service"):
            continue
        out.append({"name": name, "load": u.get("load", ""), "active": u.get("active", ""),
                    "sub": u.get("sub", ""), "description": u.get("description", ""),
                    "scope": scope, "status": service_status(u.get("active", ""),
                                                             u.get("sub", ""))})
    return out


def service_status(active, sub):
    if active == "failed" or sub == "failed":
        return "Failed"
    if sub == "running":
        return "Running"
    if active == "active" and sub == "exited":
        return "Exited"
    if active in ("activating", "reloading"):
        return "Starting"
    if active == "deactivating":
        return "Stopping"
    return "Stopped"


def parse_show(text):
    """{Id: MainPID} from `systemctl show -p MainPID,Id a b c`."""
    out = {}
    cur = {}
    for line in (text or "").splitlines() + [""]:
        if not line.strip():
            if "Id" in cur:
                try:
                    out[cur["Id"]] = int(cur.get("MainPID", "0") or 0)
                except ValueError:
                    out[cur["Id"]] = 0
            cur = {}
            continue
        k, _, v = line.partition("=")
        cur[k] = v
    return out


def service_argv(action, name, scope):
    argv = ["systemctl"]
    if scope == "user":
        argv.append("--user")
    return argv + [action, name]
