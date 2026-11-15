//! Standalone installed-applications inventory (apt / snap / flatpak / local),
//! independent of App Hub. Data comes from the package manager's own cache,
//! `dpkg-query`, `snap list`, `flatpak list` and the `.desktop` database.
//! Uninstalls run through pkexec directly — no App Hub helper required.

use std::process::{Command, Stdio};
use std::sync::mpsc::Receiver;

use crate::jobs::{self, JobEvent};

#[derive(Clone, Default)]
pub struct SysApp {
    /// apt | snap | flatpak | local
    pub channel: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub source: String,
    pub size_bytes: i64,
    /// Absolute icon path or a themed icon name.
    pub icon: Option<String>,
    pub protected: Option<String>,
    /// flatpak: system | user.
    pub installation: String,
    /// flatpak branch (e.g. "stable").
    pub branch: String,
    /// Path of the launcher .desktop file, if any.
    pub desktop: String,
    /// For channel "local": the file the launcher runs (used for trash-based uninstall).
    pub exec_target: String,
}

impl SysApp {
    pub fn channel_label(&self) -> &'static str {
        match self.channel.as_str() {
            "apt" => "Deb",
            "snap" => "Snap",
            "flatpak" => "Flatpak",
            "local" => "Local",
            _ => "App",
        }
    }

    pub fn flatpak_ref(&self) -> String {
        if self.branch.is_empty() {
            self.id.clone()
        } else {
            format!("{}//{}", self.id, self.branch)
        }
    }

    pub fn size_label(&self) -> String {
        if self.size_bytes <= 0 {
            return String::new();
        }
        crate::cleaner::human_bytes(self.size_bytes)
    }

    /// Whether the launcher's program file may be moved to the Trash:
    /// only files inside the user's home directory. Anything else
    /// (system binaries, shared executables) is left alone.
    pub fn local_target_trashable(&self) -> bool {
        if self.channel != "local" || self.exec_target.is_empty() {
            return false;
        }
        let home = std::env::var("HOME").unwrap_or_default();
        if home.is_empty() {
            return false;
        }
        let target = std::path::Path::new(&self.exec_target);
        match target.canonicalize() {
            Ok(resolved) => resolved.starts_with(&home),
            Err(_) => target.is_absolute() && target.starts_with(&home),
        }
    }

    pub fn uninstallable(&self) -> bool {
        if self.protected.is_some() {
            return false;
        }
        match self.channel.as_str() {
            // A local app can be trashed only if we know what file to trash.
            "local" => self.local_target_trashable(),
            _ => true,
        }
    }

}

// The scan script intentionally does everything in one python process:
// it is read-only (besides open file reads) and takes a few seconds.
const SCAN_SCRIPT: &str = r#"
import glob, json, os, re, subprocess, sys

home = os.path.expanduser('~')
data_home = os.environ.get('XDG_DATA_HOME') or os.path.join(home, '.local/share')

def parse_desktop(path, nodisplay_ok=False):
    try:
        with open(path, encoding='utf-8', errors='ignore') as fh:
            text = fh.read()
    except OSError:
        return None
    entry, in_entry = {}, False
    for raw in text.splitlines():
        line = raw.strip()
        if line.startswith('['):
            in_entry = line == '[Desktop Entry]'
            continue
        if not in_entry or '=' not in line or line.startswith('#'):
            continue
        k, v = line.split('=', 1)
        k = k.strip()
        if k not in entry:
            entry[k] = v.strip()
    if entry.get('Type') != 'Application':
        return None
    if entry.get('Hidden', '').lower() == 'true':
        return None
    if entry.get('NoDisplay', '').lower() == 'true' and not nodisplay_ok:
        return None
    if entry.get('Name', '').strip() == '':
        return None
    return entry

def exec_target(entry):
    import shlex, shutil
    raw = entry.get('Exec', '')
    try:
        parts = shlex.split(raw)
    except ValueError:
        parts = raw.split()
    if not parts:
        return ''
    i = 0
    while i < len(parts):
        tok = parts[i]
        if tok.startswith('env') and len(parts) > i + 1:
            i += 1
            continue
        if '=' in tok and not os.path.isabs(tok):
            i += 1
            continue
        break
    if i >= len(parts):
        return ''
    t = parts[i]
    if os.path.isabs(t) and os.path.isfile(t):
        return t
    p = shutil.which(t)
    return p or ''

apps = []
def add(**kw):
    apps.append(kw)

# ---------------- apt ----------------
try:
    import apt
    cache = apt.Cache()
except Exception:
    cache = None

files = glob.glob('/usr/share/applications/*.desktop')
mapping = {}
if files:
    try:
        out = subprocess.run(['dpkg-query', '-S', *files],
                             capture_output=True, text=True, timeout=60).stdout
        for line in out.splitlines():
            if ' from: ' in line or ':' not in line:
                continue
            pkgs, path = line.split(':', 1)
            pkg = pkgs.split(',')[0].strip()
            mapping[path.strip()] = pkg
    except Exception:
        mapping = {}

APT_PROTECTED = {'app-hub', 'apt', 'dpkg', 'sudo', 'systemd', 'polkitd', 'pkexec', 'gdm3',
                 'gnome-shell', 'gnome-session-bin', 'ubuntu-session', 'network-manager',
                 'snapd', 'flatpak', 'xwayland'}

for path in files:
    pkg = mapping.get(path)
    if not pkg:
        continue
    entry = parse_desktop(path)
    if not entry:
        continue
    inst = None
    package = cache[pkg] if (cache and pkg in cache) else None
    if package is not None:
        inst = package.installed
    source = 'Local .deb'
    try:
        for o in (list(inst.origins) if inst else []):
            if o.archive == 'now':
                continue
            label = o.label or 'Ubuntu'
            comp = '/' + o.component if o.component else ''
            source = f'{label} {o.archive}{comp}'
            break
    except Exception:
        pass
    protected = None
    if pkg in APT_PROTECTED or pkg.startswith('ubuntu-'):
        protected = 'This is a core system package'
    elif package is not None and package.essential:
        protected = 'Required by the system'
    elif inst is not None and inst.priority == 'required':
        protected = 'Required by the system'
    add(channel='apt', id=pkg, name=entry.get('Name', pkg),
        version=inst.version if inst else '', source=source,
        size=int(inst.installed_size) if inst else 0, icon=entry.get('Icon', ''),
        protected=protected or '', installation='', branch='', desktop=path,
        exec_target='')

# ---------------- snap ----------------
try:
    r = subprocess.run(['snap', 'list'], capture_output=True, text=True, timeout=30)
    if r.returncode == 0:
        snap_desktops = {}
        for d in glob.glob('/var/lib/snapd/desktop/applications/*.desktop'):
            stem = os.path.basename(d)[:-8]
            snap_name = stem.split('_', 1)[0]
            snap_desktops.setdefault(snap_name, []).append(d)
        for line in r.stdout.splitlines()[1:]:
            cols = line.split()
            if len(cols) < 5:
                continue
            name, version, rev = cols[0], cols[1], cols[2]
            tracking = cols[3]
            publisher = ' '.join(cols[4:-1])
            desk_path, entry = '', {}
            for d in snap_desktops.get(name, []):
                e = parse_desktop(d, nodisplay_ok=True)
                if e:
                    desk_path, entry = d, e
                    if os.path.basename(d)[:-8] == name + '_' + name:
                        break
            size = 0
            try:
                size = os.path.getsize(f'/var/lib/snapd/snaps/{name}_{rev}.snap')
            except OSError:
                pass
            add(channel='snap', id=name, name=entry.get('Name', name),
                version=version, source=f'{tracking} · {publisher}',
                size=size, icon=entry.get('Icon', ''),
                protected='', installation='', branch='', desktop=desk_path,
                exec_target='')
except Exception:
    pass

# ---------------- flatpak ----------------
try:
    r = subprocess.run(['flatpak', 'list', '--app',
                        '--columns=application,branch,version,origin,installation'],
                       capture_output=True, text=True, timeout=30)
    if r.returncode == 0:
        for line in r.stdout.splitlines():
            if not line.strip():
                continue
            cols = line.split('\t')
            if len(cols) < 4:
                cols = re.split(r'\s{2,}', line.strip())
            if len(cols) < 4:
                continue
            fid, branch, version, origin = cols[0], cols[1], cols[2], cols[3]
            installation = cols[4] if len(cols) > 4 else 'system'
            if installation == 'system':
                deskp = f'/var/lib/flatpak/exports/share/applications/{fid}.desktop'
                icon_base = '/var/lib/flatpak/exports/share/icons'
            else:
                deskp = os.path.join(home, '.local/share/flatpak/exports/share/applications/' + fid + '.desktop')
                icon_base = os.path.join(home, '.local/share/flatpak/exports/share/icons')
            entry = parse_desktop(deskp, nodisplay_ok=True) if os.path.isfile(deskp) else {}
            icon = entry.get('Icon', '')
            if icon and not os.path.isabs(icon):
                found = ''
                for ext in ('.png', '.svg'):
                    hits = glob.glob(os.path.join(icon_base, '**', icon + ext), recursive=True)
                    if hits:
                        found = hits[0]
                        break
                icon = found
            size = 0
            try:
                ri = subprocess.run(['flatpak', 'info', '--show-size', fid],
                                    capture_output=True, text=True, timeout=30)
                size = int((ri.stdout or '0').split()[0] or '0')
            except Exception:
                pass
            add(channel='flatpak', id=fid, name=entry.get('Name', fid),
                version=version, source=f'{origin} · {installation}',
                size=size, icon=icon, protected='',
                installation=installation, branch=branch,
                desktop=deskp if os.path.isfile(deskp) else '', exec_target='')
except Exception:
    pass

# ---------------- local apps ----------------
seen = {a['desktop'] for a in apps if a['desktop']}
for d in [os.path.join(data_home, 'applications')]:
    for path in glob.glob(os.path.join(d, '*.desktop')):
        if path in seen:
            continue
        entry = parse_desktop(path, nodisplay_ok=True)
        if not entry:
            continue
        target = exec_target(entry)
        size = 0
        try:
            size = os.path.getsize(target) if target else 0
        except OSError:
            pass
        add(channel='local', id=os.path.basename(path)[:-8],
            name=entry.get('Name', os.path.basename(path)[:-8]),
            version='', source='Local file', size=size,
            icon=entry.get('Icon', ''), protected='', installation='',
            branch='', desktop=path, exec_target=target)

apps.sort(key=lambda a: a['name'].lower())
print(json.dumps(apps))
"#;

/// Installed applications across every channel. Blocking call — run on a
/// background thread (dpkg/apt/flatpak are all queried once).
pub fn list() -> Result<Vec<SysApp>, String> {
    let output = Command::new("python3")
        .args(["-c", SCAN_SCRIPT])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Could not run the inventory scan: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if err.is_empty() {
            "The inventory scan failed.".to_owned()
        } else {
            err
        });
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(text.trim())
        .map_err(|e| format!("Could not parse the inventory: {e}"))?;
    let Some(items) = json.as_array() else {
        return Err("The inventory returned no data.".to_owned());
    };
    let mut apps = Vec::with_capacity(items.len());
    for item in items {
        let text = |key: &str| {
            item.get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned()
        };
        let icon = text("icon");
        let protected = text("protected");
        apps.push(SysApp {
            channel: text("channel"),
            id: text("id"),
            name: text("name"),
            version: text("version"),
            source: text("source"),
            size_bytes: item.get("size").and_then(|v| v.as_i64()).unwrap_or(0),
            icon: if icon.is_empty() { None } else { Some(icon) },
            protected: if protected.is_empty() {
                None
            } else {
                Some(protected)
            },
            installation: text("installation"),
            branch: text("branch"),
            desktop: text("desktop"),
            exec_target: text("exec_target"),
        });
    }
    Ok(apps)
}

/// Root script shared by [`remove`], the uninstaller confirmation flow and the
/// cleaner: one pkexec prompt, one python process doing the privileged work.
pub const ROOT_SCRIPT: &str = r#"
import json, subprocess, sys
sel = json.loads(sys.argv[1])
rc = 0

def run(cmd):
    global rc
    print('$ ' + ' '.join(cmd), flush=True)
    r = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    for line in (r.stdout or '').splitlines():
        print(line, flush=True)
    if r.returncode != 0 and rc == 0:
        rc = r.returncode

apt_remove = sel.get('apt_remove') or []
if apt_remove:
    run(['apt-get', 'remove', '-y'] + list(apt_remove))
apt_purge = sel.get('apt_purge') or []
if apt_purge:
    run(['apt-get', 'purge', '-y'] + list(apt_purge))
if sel.get('apt_autoremove'):
    run(['apt-get', 'autoremove', '-y'])
if sel.get('apt_clean'):
    run(['apt-get', 'clean'])
for name, rev in sel.get('snap_remove') or []:
    cmd = ['snap', 'remove']
    if rev:
        cmd.append(f'--revision={rev}')
    cmd.append(name)
    run(cmd)
for ref in sel.get('flatpak_remove') or []:
    run(['flatpak', 'uninstall', '--system', '-y', '--noninteractive', ref])
if sel.get('flatpak_unused_system'):
    run(['flatpak', 'uninstall', '--system', '--unused', '-y', '--noninteractive'])
sys.exit(rc)
"#;

/// Simulate removing an apt package to show what else would disappear.
/// Returns (all_removed, risky_removed) package names.
pub fn apt_removal_preview(package: &str) -> (Vec<String>, Vec<String>) {
    let output = Command::new("apt-get")
        .args(["-s", "remove", package])
        .output();
    let Ok(output) = output else {
        return (Vec::new(), Vec::new());
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut removed = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Remv ") {
            if let Some(pkg) = rest.split_whitespace().next() {
                removed.push(pkg.to_owned());
            }
        }
    }
    let risky = [
        "app-hub", "apt", "dpkg", "sudo", "systemd", "polkitd", "pkexec", "gdm3",
        "gnome-shell", "gnome-session-bin", "ubuntu-session", "network-manager",
        "snapd", "flatpak", "xwayland",
    ];
    let risky_removed = removed
        .iter()
        .filter(|pkg| risky.iter().any(|r| *pkg == r) || pkg.starts_with("ubuntu-"))
        .cloned()
        .collect();
    (removed, risky_removed)
}

fn plan_json(map: &[(&str, serde_json::Value)]) -> String {
    let mut obj = serde_json::Map::new();
    for (key, value) in map {
        obj.insert((*key).to_owned(), value.clone());
    }
    serde_json::Value::Object(obj).to_string()
}

/// Uninstall one app. apt/snap/system-flatpak run through one pkexec prompt;
/// user flatpaks and local apps run unprivileged (gio trash / flatpak --user).
/// Output streams line-by-line into the activity log.
pub fn remove(app: &SysApp) -> Receiver<JobEvent> {
    match app.channel.as_str() {
        "apt" => {
            let plan = plan_json(&[("apt_remove", serde_json::json!([app.id]))]);
            pkexec_root_plan(&plan)
        }
        "snap" => {
            let plan = plan_json(&[("snap_remove", serde_json::json!([[app.id, ""]]))]);
            pkexec_root_plan(&plan)
        }
        "flatpak" if app.installation == "user" => {
            let mut cmd = Command::new("flatpak");
            cmd.args([
                "uninstall",
                "--user",
                "-y",
                "--noninteractive",
                &app.flatpak_ref(),
            ]);
            cmd.stdin(Stdio::null());
            jobs::spawn(cmd)
        }
        "flatpak" => {
            let plan = plan_json(&[("flatpak_remove", serde_json::json!([app.flatpak_ref()]))]);
            pkexec_root_plan(&plan)
        }
        "local" => {
            // gio trash both files; recoverable from the Trash. The program
            // file is only trashed when it lives in the user's home —
            // system files (gapplication, bash, …) are left alone.
            let desk = shell_quote(&app.desktop);
            let script = if app.local_target_trashable() {
                let target = shell_quote(&app.exec_target);
                format!(
                    "gio trash -- {desk}; rc1=$?; gio trash -- {target}; rc2=$?; \
                     if [ $rc1 -ne 0 ] || [ $rc2 -ne 0 ]; then exit 1; fi"
                )
            } else {
                format!(
                    "gio trash -- {desk} || exit 1; \
                     echo 'The program file is a system file — only the launcher was moved to the Trash.'"
                )
            };
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg(script);
            cmd.stdin(Stdio::null());
            jobs::spawn(cmd)
        }
        _ => {
            let mut cmd = Command::new("python3");
            cmd.args([
                "-c",
                "import sys; print(sys.argv[1]); sys.exit(1)",
                &format!("Uninstalling {} apps is not supported.", app.channel_label()),
            ]);
            cmd.stdin(Stdio::null());
            jobs::spawn(cmd)
        }
    }
}

pub fn pkexec_root_plan(plan: &str) -> Receiver<JobEvent> {
    let mut cmd = Command::new("pkexec");
    cmd.args(["python3", "-c", ROOT_SCRIPT, plan]);
    cmd.stdin(Stdio::null());
    jobs::spawn(cmd)
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
