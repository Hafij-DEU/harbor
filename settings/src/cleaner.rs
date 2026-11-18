//! "Free Up Space" — a standalone cleaner (no App Hub required).
//! Finds: downloaded package files, unused dependencies, leftover /etc
//! configs of removed packages, old snap revisions, unused Flatpak runtimes
//! and app data left behind by removed Flatpak/Snap apps. User files go to
//! the Trash (recoverable); privileged work runs through one pkexec prompt.

use std::process::{Command, Stdio};
use std::sync::mpsc::Receiver;

use crate::inventory::ROOT_SCRIPT;
use crate::jobs::{self, JobEvent};

#[derive(Clone, Default)]
pub struct CleanerItem {
    pub label: String,
    pub detail: String,
    pub size: i64,
    pub selected: bool,
    pub path: String,
    /// Root-plan step for this item (granular root groups only):
    /// ("apt_purge", pkg) or ("snap_remove", name, rev)
    pub step: Vec<String>,
}

#[derive(Clone, Default)]
pub struct CleanerGroup {
    pub title: String,
    pub description: String,
    pub granular: bool,
    pub action: String,
    /// Non-granular groups: whole-group action, keyed into the root plan.
    pub root_plan: Vec<Vec<String>>,
    pub user_cmds: Vec<Vec<String>>,
    pub note: String,
    pub items: Vec<CleanerItem>,
    pub enabled: bool,
}

impl CleanerGroup {
    pub fn size(&self) -> i64 {
        self.items.iter().map(|i| i.size).sum()
    }

    pub fn selected_size(&self) -> i64 {
        if self.granular {
            self.items
                .iter()
                .filter(|i| i.selected)
                .map(|i| i.size)
                .sum()
        } else if self.enabled {
            self.size()
        } else {
            0
        }
    }
}

const SCAN_SCRIPT: &str = r#"
import glob, json, os, re, subprocess, sys
import shlex

home = os.path.expanduser('~')
out = []

def dir_size(path):
    total = 0
    if os.path.isfile(path) or os.path.islink(path):
        try:
            return os.lstat(path).st_size
        except OSError:
            return 0
    for root, _dirs, files in os.walk(path, onerror=lambda e: None):
        for name in files:
            try:
                total += os.lstat(os.path.join(root, name)).st_size
            except OSError:
                pass
    return total

def installed_flatpak_ids():
    ids = set()
    try:
        r = subprocess.run(['flatpak', 'list', '--app', '--columns=application,installation'],
                           capture_output=True, text=True, timeout=30)
        for line in r.stdout.splitlines():
            cols = line.split('\t')
            if cols and cols[0].strip():
                ids.add(cols[0].strip())
    except Exception:
        pass
    return ids

def installed_snaps():
    names = set()
    try:
        r = subprocess.run(['snap', 'list'], capture_output=True, text=True, timeout=30)
        for line in r.stdout.splitlines()[1:]:
            if line.split():
                names.add(line.split()[0])
    except Exception:
        pass
    return names

def rc_packages():
    try:
        r = subprocess.run(['dpkg-query', '-W', '-f=${db:Status-Abbrev} ${Package}\n'],
                           capture_output=True, text=True, timeout=60)
        return [line.split()[1] for line in r.stdout.splitlines() if line.startswith('rc')]
    except Exception:
        return []

def apt_size(name):
    try:
        import apt
        cache = apt.Cache()
        if name in cache and cache[name].installed:
            return int(cache[name].installed.installed_size)
    except Exception:
        pass
    return 0

def apt_autoremove():
    try:
        r = subprocess.run(['apt-get', '-s', 'autoremove'],
                           capture_output=True, text=True, timeout=120)
        names = [line.split()[1] for line in r.stdout.splitlines()
                 if line.startswith('Remv ')]
        return names
    except Exception:
        return []

def snap_old_revisions():
    items = []
    try:
        r = subprocess.run(['snap', 'list', '--all'], capture_output=True, text=True, timeout=30)
        for line in r.stdout.splitlines()[1:]:
            cols = line.split()
            if len(cols) < 6:
                continue
            name, rev = cols[0], cols[2]
            notes = cols[-1]
            if 'disabled' not in notes:
                continue
            path = f'/var/lib/snapd/snaps/{name}_{rev}.snap'
            size = os.path.getsize(path) if os.path.exists(path) else 0
            items.append({'label': f'{name} (revision {rev})',
                          'detail': 'disabled revision kept after an update',
                          'size': size, 'path': '',
                          'step': ['snap_remove', name, rev]})
    except Exception:
        pass
    return items

def flatpak_unused():
    found, scopes = [], []
    flatpak = None
    from shutil import which
    flatpak = which('flatpak')
    if not flatpak:
        return found, scopes
    for scope in ('--system', '--user'):
        try:
            r = subprocess.run([flatpak, 'uninstall', scope, '--unused'],
                               input='n\n', capture_output=True, text=True,
                               timeout=60)
        except Exception:
            continue
        refs = re.findall(r'^\s*\d+\.\s+(?:\[\S+\]\s+)?(\S+)\s+(\S+)', r.stdout, re.MULTILINE)
        if refs:
            scopes.append(scope)
            found += [{'label': ref, 'detail': f'{branch} · {scope[2:]}',
                       'size': 0, 'path': '', 'step': ['flatpak_unused', scope]} for ref, branch in refs]
    return found, scopes

def orphaned_app_data():
    items = []
    fp = installed_flatpak_ids()
    for path in glob.glob(os.path.join(home, '.var/app/*')):
        fid = os.path.basename(path)
        if fid not in fp:
            try:
                size = dir_size(path)
            except Exception:
                size = 0
            if size > 0:
                items.append({'label': fid, 'detail': '~/.var/app — app data of an uninstalled Flatpak',
                              'size': size, 'path': path, 'step': []})
    snaps = installed_snaps()
    for path in glob.glob(os.path.join(home, 'snap/*/')):
        name = os.path.basename(path.rstrip('/'))
        if name not in snaps:
            try:
                size = dir_size(path)
            except Exception:
                size = 0
            if size > 0:
                items.append({'label': name, 'detail': '~/snap — data of an uninstalled snap',
                              'size': size, 'path': path, 'step': []})
    return items

def apt_archives():
    files = glob.glob('/var/cache/apt/archives/*.deb')
    return [{'label': os.path.basename(f), 'detail': '', 'size': os.path.getsize(f),
             'path': '', 'step': []} for f in files]

archives = apt_archives()
out.append({'title': 'Downloaded package files',
            'description': 'Old .deb files in /var/cache/apt/archives. apt downloads them again if needed.',
            'granular': False, 'action': 'root', 'items': archives,
            'root_plan': 'apt_clean', 'user_cmds': []})

auto = apt_autoremove()
out.append({'title': 'Unused dependencies',
            'description': 'Packages installed only as dependencies of apps that are gone (apt autoremove).',
            'granular': False, 'action': 'root',
            'items': [{'label': n, 'detail': '', 'size': apt_size(n), 'path': '', 'step': []}
                      for n in auto],
            'root_plan': 'apt_autoremove', 'user_cmds': []})

residual = rc_packages()
out.append({'title': 'Leftover system settings',
            'description': 'Configuration files in /etc kept after removing these packages (dpkg “rc” state).',
            'granular': True, 'action': 'root',
            'items': [{'label': p, 'detail': 'dpkg “rc” — config kept by the package manager',
                       'size': 0, 'path': '', 'step': ['apt_purge', p]} for p in residual],
            'root_plan': '', 'user_cmds': []})

old_snaps = snap_old_revisions()
out.append({'title': 'Old Snap versions',
            'description': 'Disabled revisions kept after snap updates.',
            'granular': True, 'action': 'root', 'items': old_snaps,
            'root_plan': '', 'user_cmds': []})

unused, scopes = flatpak_unused()
out.append({'title': 'Unused Flatpak runtimes',
            'description': 'Runtimes and extensions no installed Flatpak app needs.',
            'granular': False, 'action': 'root', 'items': unused,
            'root_plan': 'flatpak_unused_system' if '--system' in scopes else '',
            'user_cmds': [(['flatpak', 'uninstall', '--user', '--unused', '-y', '--noninteractive'])]
                          if '--user' in scopes else []})

orphans = orphaned_app_data()
out.append({'title': 'App data of removed applications',
            'description': 'Settings and data of Flatpak and snap apps that are no longer installed. Moved to the Trash.',
            'granular': True, 'action': 'trash', 'items': orphans,
            'root_plan': '', 'user_cmds': []})

out = [g for g in out if g['items']]
print(json.dumps(out))
"#;

/// Scan the system for cleanable space. Blocking; run on a background thread.
pub fn scan() -> Result<Vec<CleanerGroup>, String> {
    let output = Command::new("python3")
        .args(["-c", SCAN_SCRIPT])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Could not run the cleaner scan: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if err.is_empty() {
            "The cleaner scan failed.".to_owned()
        } else {
            err
        });
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(text.trim())
        .map_err(|e| format!("Could not parse the cleaner scan: {e}"))?;
    let Some(groups) = json.as_array() else {
        return Err("The cleaner returned no data.".to_owned());
    };
    let mut result = Vec::new();
    for g in groups {
        let items = g
            .get("items")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|i| CleanerItem {
                label: i
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned(),
                detail: i
                    .get("detail")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned(),
                size: i.get("size").and_then(|v| v.as_i64()).unwrap_or(0),
                selected: true,
                path: i
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned(),
                step: i
                    .get("step")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
            .collect();
        result.push(CleanerGroup {
            title: g
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
            description: g
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
            granular: g.get("granular").and_then(|v| v.as_bool()).unwrap_or(true),
            action: g
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
            root_plan: g
                .get("root_plan")
                .and_then(|v| v.as_str())
                .map(|s| {
                    if s.is_empty() {
                        Vec::new()
                    } else {
                        vec![vec![s.to_owned()]]
                    }
                })
                .unwrap_or_default(),
            user_cmds: g
                .get("user_cmds")
                .and_then(|v| v.as_array())
                .map(|cmds| {
                    cmds.iter()
                        .filter_map(|s| {
                            s.as_array().map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(str::to_owned))
                                    .collect()
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
            note: g
                .get("note")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
            items,
            enabled: true,
        });
    }
    Ok(result)
}

/// The user's pick from a scan, ready for [`execute`].
pub struct CleanerSelection {
    pub trash: Vec<String>,
    pub plan: serde_json::Map<String, serde_json::Value>,
    pub user: Vec<Vec<String>>,
    pub selected_bytes: i64,
}

pub fn selection(groups: &[CleanerGroup]) -> CleanerSelection {
    let mut trash = Vec::new();
    let mut plan = serde_json::Map::new();
    let mut user: Vec<Vec<String>> = Vec::new();
    let mut selected_bytes = 0i64;
    let mut apt_purge: Vec<String> = Vec::new();
    let mut snap_remove: Vec<(String, String)> = Vec::new();

    for g in groups {
        if g.granular {
            for item in &g.items {
                if !item.selected {
                    continue;
                }
                selected_bytes += item.size;
                match g.action.as_str() {
                    "trash" => {
                        if !item.path.is_empty() {
                            trash.push(item.path.clone());
                        }
                    }
                    "root" => match item.step.first().map(String::as_str) {
                        Some("apt_purge") => {
                            if let Some(pkg) = item.step.get(1) {
                                apt_purge.push(pkg.clone());
                            }
                        }
                        Some("snap_remove") => {
                            if let (Some(name), Some(rev)) = (item.step.get(1), item.step.get(2)) {
                                snap_remove.push((name.clone(), rev.clone()));
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        } else if g.enabled {
            selected_bytes += g.size();
            for step in &g.root_plan {
                match step.first().map(String::as_str) {
                    Some("apt_clean") => {
                        plan.insert("apt_clean".to_owned(), serde_json::json!(true));
                    }
                    Some("apt_autoremove") => {
                        plan.insert("apt_autoremove".to_owned(), serde_json::json!(true));
                    }
                    Some("flatpak_unused_system") => {
                        plan.insert("flatpak_unused_system".to_owned(), serde_json::json!(true));
                    }
                    _ => {}
                }
            }
            user.extend(g.user_cmds.iter().cloned());
        }
    }
    if !apt_purge.is_empty() {
        plan.insert("apt_purge".to_owned(), serde_json::json!(apt_purge));
    }
    if !snap_remove.is_empty() {
        plan.insert(
            "snap_remove".to_owned(),
            serde_json::json!(snap_remove
                .iter()
                .map(|(n, r)| vec![n.clone(), r.clone()])
                .collect::<Vec<_>>()),
        );
    }
    CleanerSelection {
        trash,
        plan,
        user,
        selected_bytes,
    }
}

pub fn execute(sel: &CleanerSelection) -> Receiver<JobEvent> {
    let plan_json = serde_json::Value::Object(sel.plan.clone()).to_string();
    let trash = serde_json::to_string(&sel.trash).unwrap_or_else(|_| "[]".to_owned());
    let user = serde_json::to_string(&sel.user).unwrap_or_else(|_| "[]".to_owned());
    let root_script = ROOT_SCRIPT;

    // Everything user-side first (Trash, --user commands); then one pkexec
    // prompt for the root plan, streaming its output.
    let script = r#"
import json, os, subprocess, sys

trash = json.loads(sys.argv[1])
user = json.loads(sys.argv[2])
plan_json, root_script = sys.argv[3], sys.argv[4]
need_root = bool(json.loads(plan_json))

rc = 0
for path in trash:
    r = subprocess.run(['gio', 'trash', '--', path], capture_output=True, text=True)
    if r.returncode == 0:
        print(f'moved to Trash: {path}', flush=True)
    else:
        rc = rc or r.returncode
        print(f'could not trash {path}: {r.stderr.strip()}', flush=True)

for cmd in user:
    print('$ ' + ' '.join(cmd), flush=True)
    r = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    for line in (r.stdout or '').splitlines():
        print(line, flush=True)
    if r.returncode != 0:
        rc = rc or r.returncode

if need_root:
    r = subprocess.run(['pkexec', 'python3', '-c', root_script, plan_json],
                       stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    for line in (r.stdout or '').splitlines():
        print(line, flush=True)
    if r.returncode != 0:
        rc = rc or r.returncode

print('Free-up-space finished.', flush=True)
sys.exit(rc)
"#;
    jobs::spawn({
        let mut cmd = Command::new("python3");
        cmd.args(["-c", script, &trash, &user, &plan_json, root_script]);
        cmd.stdin(Stdio::null());
        cmd
    })
}

pub fn human_bytes(bytes: i64) -> String {
    if bytes <= 0 {
        return "0 B".to_owned();
    }
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < units.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} B", bytes)
    } else if size >= 100.0 {
        format!("{size:.0} {}", units[unit])
    } else {
        format!("{size:.1} {}", units[unit])
    }
}
