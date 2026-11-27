"""Glint core: catalogs and logic shared by the GTK app and the root helper.

Nothing here imports GTK. The root helper imports this module from its own
install directory, so every command a root operation can run is defined in
this file; the app only ever sends operation IDs to the helper.
"""
import glob
import json
import os
import re
import shutil
import socket
import struct
import subprocess
import time
from dataclasses import dataclass, field

APP_ID = "io.github.hafij_deu.Glint"
HELPER_PATH = "/usr/lib/glint/glint-helper"

PKG_RE = re.compile(r"^[a-z0-9][a-z0-9+.\-]{0,99}$")
FLATPAK_RE = re.compile(r"^[A-Za-z0-9_\-]+(\.[A-Za-z0-9_\-]+){2,}$")

# Packages a debloat removal must never take with it. The helper simulates
# every removal first and refuses when one of these would go.
PROTECTED_PKGS = {
    "ubuntu-desktop", "ubuntu-desktop-minimal", "gnome-shell", "gdm3",
    "network-manager", "snapd", "apt", "dpkg", "sudo", "pkexec", "polkitd",
    "systemd", "gnome-control-center", "nautilus", "gnome-terminal",
    "ptyxis", "xdg-desktop-portal", "pipewire", "wireplumber",
    "ubuntu-minimal", "ubuntu-standard", "language-selector-common",
    "ubuntu-release-upgrader-core", "update-notifier-common", "update-manager-core",
    "linux-generic", "linux-image-generic", "linux-headers-generic", "flatpak",
    "xwayland", "mutter", "libinput10", "gnome-session", "gnome-settings-daemon",
}
PROTECTED_PREFIXES = ("linux-generic", "linux-image-generic", "linux-signed-generic")


def is_protected(pkg, running_kernel=None):
    if pkg in PROTECTED_PKGS or pkg.startswith(PROTECTED_PREFIXES):
        return True
    if running_kernel:
        v = kernel_version(running_kernel)
        if v and kernel_pkg_version(pkg) == v:
            return True
    return False


def run(argv, timeout=30, env=None):
    """Run argv, return (code, stdout+stderr). Never raises on failure."""
    try:
        p = subprocess.run(argv, capture_output=True, text=True,
                           timeout=timeout, env=env)
        return p.returncode, (p.stdout or "") + (p.stderr or "")
    except (OSError, subprocess.SubprocessError) as e:
        return 127, str(e)


def have(cmd):
    return shutil.which(cmd) is not None


# --------------------------------------------------------------------------
# Tweaks
# --------------------------------------------------------------------------
# kind:
#   gset      user, data = [(schema, key, value)]   applied when all equal
#   umask     user, data = unit                     applied when masked
#   svc_off   root, data = [units]                  applied when none enabled
#   svc_on    root, data = [units]                  applied when all enabled
#   dropin    root, data = (path, content, reload)  applied when file matches
#   cmd       root, data = (check, apply, revert)   shell strings from here
#   autostart user, data = .desktop name            applied when hidden for this user


@dataclass
class Tweak:
    id: str
    cat: str
    title: str
    desc: str
    kind: str
    data: object
    rec: bool = False
    warn: str = ""

    @property
    def root(self):
        return self.kind in ("svc_off", "svc_on", "dropin", "cmd")


TWEAK_CATS = [
    ("privacy", "Privacy", "Crash reports, usage data and news"),
    ("performance", "Performance", "Memory, journal, boot and shutdown"),
    ("updates", "Updates", "When snaps and packages update themselves"),
    ("network", "Network", "Speed and privacy of your connection"),
    ("services", "Services", "Background services you may not need"),
    ("startup", "Startup", "Background helpers that start when you log in"),
    ("desktop", "Desktop", "GNOME behaviour and comfort"),
]

TWEAKS = [
    # Privacy
    Tweak("apport", "privacy", "Turn off crash reporter (Apport)",
          "Stops crash dialogs and crash dumps in /var/crash.",
          "svc_off", ["apport.service"], rec=True),
    Tweak("whoopsie", "privacy", "Turn off error uploads (Whoopsie)",
          "Stops sending crash reports to Canonical.",
          "svc_off", ["whoopsie.service", "whoopsie.path"], rec=True),
    Tweak("motd-news", "privacy", "Turn off MOTD news",
          "Stops the login banner from fetching news from Canonical.",
          "cmd", ("grep -qs '^ENABLED=0' /etc/default/motd-news",
                  "sed -i 's/^ENABLED=.*/ENABLED=0/' /etc/default/motd-news",
                  "sed -i 's/^ENABLED=.*/ENABLED=1/' /etc/default/motd-news"),
          rec=True),
    Tweak("apt-news", "privacy", "Hide Ubuntu Pro adverts in apt",
          "Stops apt from showing Ubuntu Pro news and offers.",
          "cmd", ("pro config show apt_news 2>/dev/null | grep -qi false",
                  "pro config set apt_news=false",
                  "pro config set apt_news=true"), rec=True),
    Tweak("tech-problems", "privacy", "Do not report technical problems",
          "GNOME no longer offers to send problem reports.",
          "gset", [("org.gnome.desktop.privacy", "report-technical-problems", "false")],
          rec=True),
    Tweak("usage-stats", "privacy", "Do not send software usage stats",
          "GNOME no longer sends app usage statistics.",
          "gset", [("org.gnome.desktop.privacy", "send-software-usage-stats", "false")],
          rec=True),
    Tweak("location", "privacy", "Turn off location services",
          "Apps can no longer ask where you are.",
          "gset", [("org.gnome.system.location", "enabled", "false")]),
    Tweak("recent-files", "privacy", "Do not remember recent files",
          "File history in Files and file pickers stays empty.",
          "gset", [("org.gnome.desktop.privacy", "remember-recent-files", "false")]),
    Tweak("auto-clean", "privacy", "Empty old trash and temp files automatically",
          "GNOME removes trash and temp files older than 30 days.",
          "gset", [("org.gnome.desktop.privacy", "remove-old-trash-files", "true"),
                   ("org.gnome.desktop.privacy", "remove-old-temp-files", "true")],
          rec=True),
    # Performance
    Tweak("swappiness", "performance", "Prefer RAM over swap (swappiness 10)",
          "The kernel swaps later, so apps stay in fast memory.",
          "dropin", ("/etc/sysctl.d/90-glint-swappiness.conf",
                     "vm.swappiness = 10\n", "sysctl --system"), rec=True),
    Tweak("cache-pressure", "performance", "Keep file system caches longer",
          "Sets vfs_cache_pressure to 50, so folders open faster.",
          "dropin", ("/etc/sysctl.d/90-glint-cache.conf",
                     "vm.vfs_cache_pressure = 50\n", "sysctl --system")),
    Tweak("journal-size", "performance", "Limit system log to 200 MB",
          "The systemd journal stops growing past 200 MB.",
          "dropin", ("/etc/systemd/journald.conf.d/90-glint.conf",
                     "[Journal]\nSystemMaxUse=200M\n",
                     "systemctl restart systemd-journald"), rec=True),
    Tweak("fast-shutdown", "performance", "Faster shutdown (10 s service timeout)",
          "A hung service delays shutdown 10 s instead of 90 s.",
          "dropin", ("/etc/systemd/system.conf.d/90-glint.conf",
                     "[Manager]\nDefaultTimeoutStopSec=10s\n",
                     "systemctl daemon-reexec"), rec=True),
    Tweak("wait-online", "performance", "Do not wait for network at boot",
          "Boot no longer waits until the network is fully up.",
          "svc_off", ["NetworkManager-wait-online.service"], rec=True),
    Tweak("trim", "performance", "Weekly SSD TRIM",
          "Keeps SSD write speed high. On by default in Ubuntu.",
          "svc_on", ["fstrim.timer"], rec=True),
    Tweak("animations", "performance", "Turn off animations",
          "Windows and menus open instantly. Feels faster on old PCs.",
          "gset", [("org.gnome.desktop.interface", "enable-animations", "false")]),
    Tweak("indexer", "performance", "Turn off file indexer (LocalSearch)",
          "Saves CPU and disk use. File search in Activities gets slower.",
          "umask", "localsearch-3.service",
          warn="Search in Files and Activities only finds file names afterwards."),
    # Updates
    Tweak("snap-hold", "updates", "Pause automatic snap updates",
          "Snaps update only when you run Update Everything.",
          "cmd", ("snap refresh --time 2>/dev/null | grep -q 'hold: forever'",
                  "snap refresh --hold", "snap refresh --unhold"),
          warn="You must update snaps yourself (Utilities > Update everything)."),
    Tweak("unattended", "updates", "Turn off unattended upgrades",
          "Security updates no longer install in the background.",
          "dropin", ("/etc/apt/apt.conf.d/99glint-unattended",
                     'APT::Periodic::Unattended-Upgrade "0";\n', ""),
          warn="Remember to install security updates yourself."),
    Tweak("apt-timers", "updates", "Turn off daily apt downloads",
          "apt stops downloading package lists in the background.",
          "svc_off", ["apt-daily.timer", "apt-daily-upgrade.timer"]),
    Tweak("software-updates", "updates", "App Center: no automatic downloads",
          "GNOME Software stops downloading updates on its own.",
          "gset", [("org.gnome.software", "download-updates", "false")]),
    # Services
    Tweak("cups", "services", "Turn off printing (CUPS)",
          "Only if you never print or scan with network printers.",
          "svc_off", ["cups.service", "cups.socket", "cups.path", "cups-browsed.service"]),
    Tweak("bluetooth", "services", "Turn off Bluetooth service",
          "Only if you never use Bluetooth devices.",
          "svc_off", ["bluetooth.service"]),
    Tweak("modemmanager", "services", "Turn off ModemManager",
          "Only if you have no mobile broadband modem.",
          "svc_off", ["ModemManager.service"]),
    Tweak("avahi", "services", "Turn off network discovery (Avahi)",
          "Stops .local name lookups and printer discovery.",
          "svc_off", ["avahi-daemon.service", "avahi-daemon.socket"]),
    # Desktop
    Tweak("battery-pct", "desktop", "Show battery percentage",
          "Shows the number next to the battery icon.",
          "gset", [("org.gnome.desktop.interface", "show-battery-percentage", "true")],
          rec=True),
    Tweak("tap-click", "desktop", "Tap to click on touchpad",
          "A light tap on the touchpad clicks.",
          "gset", [("org.gnome.desktop.peripherals.touchpad", "tap-to-click", "true")]),
    Tweak("hot-corner", "desktop", "Turn off hot corner",
          "Moving the pointer to the top left no longer opens Activities.",
          "gset", [("org.gnome.desktop.interface", "enable-hot-corners", "false")]),
    Tweak("center-windows", "desktop", "Open new windows in the centre",
          "New windows open in the middle of the screen.",
          "gset", [("org.gnome.mutter", "center-new-windows", "true")]),
    Tweak("dock-minimize", "desktop", "Click dock icon to minimise",
          "Clicking the icon of the focused app minimises it.",
          "gset", [("org.gnome.shell.extensions.dash-to-dock", "click-action", "'minimize'")]),
    # ---- added in 1.2.0 ----
    # Privacy
    Tweak("firewall", "privacy", "Turn on the firewall (ufw)",
          "Blocks incoming connections nobody asked for. Outgoing traffic is not touched.",
          "cmd", ("grep -qs '^ENABLED=yes' /etc/ufw/ufw.conf",
                  "ufw --force enable", "ufw disable"), rec=True),
    Tweak("connectivity", "privacy", "Turn off connectivity checks",
          "NetworkManager stops pinging connectivity-check.ubuntu.com. Captive portals no longer pop up.",
          "dropin", ("/etc/NetworkManager/conf.d/90-glint-connectivity.conf",
                     "[connectivity]\nenabled=false\n", "systemctl reload NetworkManager")),
    Tweak("web-search", "privacy", "No online results in Activities search",
          "Activities search only looks at this PC.",
          "gset", [("org.gnome.desktop.search-providers", "disable-external", "true")]),
    Tweak("lock-notify", "privacy", "Hide notifications on the lock screen",
          "Message previews are not shown while the screen is locked.",
          "gset", [("org.gnome.desktop.notifications", "show-in-lock-screen", "false")], rec=True),
    Tweak("recent-7", "privacy", "Forget recent files after 7 days",
          "File history is kept for a week instead of forever.",
          "gset", [("org.gnome.desktop.privacy", "recent-files-max-age", "7")]),
    Tweak("app-usage", "privacy", "Do not track app usage",
          "GNOME stops recording which apps you use most.",
          "gset", [("org.gnome.desktop.privacy", "remember-app-usage", "false")]),
    Tweak("coredump-off", "privacy", "Do not keep crash dumps",
          "systemd stops saving memory dumps of crashed programs (they can hold private data).",
          "dropin", ("/etc/systemd/coredump.conf.d/90-glint.conf",
                     "[Coredump]\nStorage=none\nProcessSizeMax=0\n", "systemctl daemon-reload"),
          rec=True),
    # Performance
    Tweak("dirty-ratio", "performance", "Smoother writes to disk",
          "Writes cached data sooner (dirty_ratio 10, background 5), so big copies stall less.",
          "dropin", ("/etc/sysctl.d/90-glint-dirty.conf",
                     "vm.dirty_ratio = 10\nvm.dirty_background_ratio = 5\n", "sysctl --system")),
    Tweak("nmi-watchdog", "performance", "Turn off NMI watchdog",
          "Saves a little power and CPU time on laptops.",
          "dropin", ("/etc/sysctl.d/90-glint-watchdog.conf",
                     "kernel.nmi_watchdog = 0\n", "sysctl --system")),
    Tweak("inotify", "performance", "More file watchers for code editors",
          "Raises inotify watches to 524288 so VS Code and build tools see every file change.",
          "dropin", ("/etc/sysctl.d/90-glint-inotify.conf",
                     "fs.inotify.max_user_watches = 524288\nfs.inotify.max_user_instances = 1024\n",
                     "sysctl --system")),
    Tweak("grub-timeout", "performance", "Shorter boot menu wait (1 s)",
          "GRUB waits 1 second instead of 10 when the menu shows.",
          "dropin", ("/etc/default/grub.d/90-glint.cfg", "GRUB_TIMEOUT=1\nGRUB_RECORDFAIL_TIMEOUT=1\n",
                     "update-grub")),
    Tweak("fwupd-timer", "performance", "No daily firmware check",
          "fwupd stops checking for firmware updates every day.",
          "svc_off", ["fwupd-refresh.timer"]),
    Tweak("man-db", "performance", "No daily manual page index",
          "Skips the man-db rebuild that runs after every package install and each day.",
          "svc_off", ["man-db.timer"]),
    Tweak("motd-timer", "performance", "No MOTD news timer",
          "Stops the timer that downloads login news.",
          "svc_off", ["motd-news.timer"]),
    # Network
    Tweak("bbr", "network", "Faster TCP (BBR congestion control)",
          "Better speed on Wi-Fi and long-distance links.",
          "dropin", ("/etc/sysctl.d/90-glint-bbr.conf",
                     "net.core.default_qdisc = fq\nnet.ipv4.tcp_congestion_control = bbr\n",
                     "sysctl --system"), rec=True),
    Tweak("dot", "network", "Encrypt DNS when possible (DNS over TLS)",
          "systemd-resolved uses TLS with DNS servers that support it.",
          "dropin", ("/etc/systemd/resolved.conf.d/90-glint-dot.conf",
                     "[Resolve]\nDNSOverTLS=opportunistic\n", "systemctl restart systemd-resolved")),
    Tweak("ipv6-privacy", "network", "IPv6 privacy addresses",
          "Uses temporary IPv6 addresses so sites cannot track your device by address.",
          "dropin", ("/etc/sysctl.d/90-glint-ipv6.conf",
                     "net.ipv6.conf.all.use_tempaddr = 2\nnet.ipv6.conf.default.use_tempaddr = 2\n",
                     "sysctl --system"), rec=True),
    Tweak("wifi-powersave", "network", "Turn off Wi-Fi power saving",
          "Fixes lag and drop-outs on many Wi-Fi cards. Uses a bit more battery.",
          "dropin", ("/etc/NetworkManager/conf.d/90-glint-wifi-powersave.conf",
                     "[connection]\nwifi.powersave = 2\n", "systemctl reload NetworkManager")),
    # Services
    Tweak("kerneloops", "services", "Turn off kernel oops reporter",
          "Stops collecting kernel crash signatures.",
          "svc_off", ["kerneloops.service"], rec=True),
    Tweak("pro-services", "services", "Turn off Ubuntu Pro background services",
          "Stops ua-timer and the Pro reboot/ESM helpers.",
          "svc_off", ["ua-timer.timer", "ubuntu-advantage.service", "esm-cache.service"]),
    Tweak("cups-browsed", "services", "Turn off network printer discovery only",
          "Printing stays on; only automatic discovery of network printers stops.",
          "svc_off", ["cups-browsed.service"]),
    Tweak("geoclue", "services", "Turn off location service (GeoClue)",
          "Stops the system location daemon.",
          "svc_off", ["geoclue.service"]),
    # Startup (user autostart overrides, reversible)
    Tweak("as-software", "startup", "Do not start App Center in the background",
          "GNOME Software keeps 150-300 MB of RAM to check updates.",
          "autostart", "org.gnome.Software.desktop", rec=True),
    Tweak("as-update-notifier", "startup", "Do not start the update notifier",
          "No more 'Software Updater' popups. Update from Utilities instead.",
          "autostart", "update-notifier.desktop"),
    Tweak("as-evolution", "startup", "Do not start calendar reminders",
          "Evolution alarm notifier stops at login. Calendar reminders stop.",
          "autostart", "org.gnome.Evolution-alarm-notify.desktop"),
    Tweak("as-dejadup", "startup", "Do not start backup monitor",
          "Déjà Dup's monitor stops; scheduled backups stop too.",
          "autostart", "org.gnome.DejaDup.Monitor.desktop"),
    Tweak("as-pro", "startup", "Do not start Ubuntu Pro notices",
          "Stops Ubuntu Pro notifications at login.",
          "autostart", "ubuntu-advantage-notification.desktop", rec=True),
    Tweak("as-report", "startup", "Do not start Ubuntu report",
          "Stops the metrics prompt after upgrades.",
          "autostart", "ubuntu-report-on-upgrade.desktop", rec=True),
    Tweak("as-orca", "startup", "Do not start Orca at login",
          "Screen reader autostart is turned off (it still starts with Super+Alt+S).",
          "autostart", "orca-autostart.desktop"),
    Tweak("as-geoclue", "startup", "Do not start location agent",
          "Stops the GeoClue demo agent at login.",
          "autostart", "geoclue-demo-agent.desktop"),
    # Desktop
    Tweak("dark", "desktop", "Dark style",
          "Apps and the shell use dark colours.",
          "gset", [("org.gnome.desktop.interface", "color-scheme", "'prefer-dark'")]),
    Tweak("night-light", "desktop", "Night light",
          "Warmer screen colours from sunset to sunrise.",
          "gset", [("org.gnome.settings-daemon.plugins.color", "night-light-enabled", "true")]),
    Tweak("clock-weekday", "desktop", "Show weekday in the clock",
          "The top bar shows e.g. 'Fri 14:05'.",
          "gset", [("org.gnome.desktop.interface", "clock-show-weekday", "true")]),
    Tweak("clock-seconds", "desktop", "Show seconds in the clock",
          "The top bar clock shows seconds.",
          "gset", [("org.gnome.desktop.interface", "clock-show-seconds", "true")]),
    Tweak("win-buttons", "desktop", "Minimise and maximise buttons",
          "Title bars show all three window buttons.",
          "gset", [("org.gnome.desktop.wm.preferences", "button-layout", "':minimize,maximize,close'")]),
    Tweak("detach-dialogs", "desktop", "Movable dialog windows",
          "Dialogs are no longer glued to their parent window.",
          "gset", [("org.gnome.mutter", "attach-modal-dialogs", "false")]),
    Tweak("low-battery-saver", "desktop", "Power saver on low battery",
          "Switches to power saver automatically when the battery is low.",
          "gset", [("org.gnome.settings-daemon.plugins.power", "power-saver-profile-on-low-battery", "true")],
          rec=True),
    Tweak("hidden-files", "desktop", "Show hidden files in file pickers",
          "Open and Save dialogs show dot files.",
          "gset", [("org.gtk.gtk4.Settings.FileChooser", "show-hidden", "true")]),
    Tweak("dock-fixed", "desktop", "Dock always visible",
          "The dock no longer hides when a window touches it.",
          "gset", [("org.gnome.shell.extensions.dash-to-dock", "dock-fixed", "true")]),
    Tweak("overamp", "desktop", "Allow volume above 100%",
          "The volume slider goes up to 150%.",
          "gset", [("org.gnome.desktop.sound", "allow-volume-above-100-percent", "true")]),
]
TWEAK_BY_ID = {t.id: t for t in TWEAKS}


def _unit_state(unit, user=False):
    argv = ["systemctl"] + (["--user"] if user else []) + ["is-enabled", unit]
    code, out = run(argv, timeout=10)
    text = out.strip()
    if not text or "No such file" in text or "not-found" in text or "not found" in text:
        return "not-found"
    return text.splitlines()[-1].strip()


def _unit_exists(unit):
    return _unit_state(unit) not in ("not-found", "")


def _gs_schema_has(schema, key):
    code, out = run(["gsettings", "list-keys", schema], timeout=10)
    return code == 0 and key in out.split()


def _gs_norm(v):
    return v.strip().strip("'\"")


def autostart_user_path(name):
    base = os.environ.get("XDG_CONFIG_HOME") or os.path.join(HOME, ".config")
    return os.path.join(base, "autostart", name)


AUTOSTART_OFF = (
    'f="$1"; mkdir -p "$(dirname "$f")"; '
    'if [ -f "$f" ] && ! grep -q "^X-Glint=true" "$f"; then '
    'if grep -q "^Hidden=" "$f"; then sed -i "s/^Hidden=.*/Hidden=true/" "$f"; '
    'else printf "Hidden=true\\n" >> "$f"; fi; '
    'else printf "[Desktop Entry]\\nType=Application\\nName=off\\nHidden=true\\nX-Glint=true\\n" > "$f"; fi')
AUTOSTART_ON = (
    'f="$1"; [ -f "$f" ] || exit 0; '
    'if grep -q "^X-Glint=true" "$f"; then rm -f "$f"; '
    'else sed -i "s/^Hidden=true/Hidden=false/; s/^X-GNOME-Autostart-enabled=false/X-GNOME-Autostart-enabled=true/" "$f"; fi')


def autostart_steps(name, enable):
    return [["sh", "-c", AUTOSTART_ON if enable else AUTOSTART_OFF, "sh", autostart_user_path(name)]]


def parse_desktop(path):
    vals = {}
    group = None
    for line in read(path).splitlines():
        line = line.strip()
        if line.startswith("["):
            group = line
            continue
        if group == "[Desktop Entry]" and "=" in line and not line.startswith("#"):
            k, _, v = line.partition("=")
            vals.setdefault(k.strip(), v.strip())
    return vals


def autostart_disabled(name):
    vals = parse_desktop(autostart_user_path(name))
    return vals.get("Hidden") == "true" or vals.get("X-GNOME-Autostart-enabled") == "false"


def tweak_available(t):
    if t.kind == "autostart":
        return os.path.exists(os.path.join("/etc/xdg/autostart", t.data)) or \
            os.path.exists(autostart_user_path(t.data))
    if t.kind == "gset":
        return all(_gs_schema_has(s, k) for s, k, _ in t.data)
    if t.kind == "umask":
        return _unit_state(t.data, user=True) != "not-found"
    if t.kind in ("svc_off", "svc_on"):
        return any(_unit_exists(u) for u in t.data)
    if t.kind == "dropin":
        return os.path.isdir(os.path.dirname(os.path.dirname(t.data[0]))) or \
            os.path.isdir(os.path.dirname(t.data[0]))
    if t.kind == "cmd":
        first = t.data[1].split()[0]
        if first == "sed":
            return os.path.exists("/etc/default/motd-news")
        return have(first)
    return False


def tweak_applied(t):
    if t.kind == "autostart":
        return autostart_disabled(t.data)
    if t.kind == "gset":
        for s, k, v in t.data:
            code, out = run(["gsettings", "get", s, k], timeout=10)
            if code != 0 or _gs_norm(out) != _gs_norm(v):
                return False
        return True
    if t.kind == "umask":
        return _unit_state(t.data, user=True) == "masked"
    if t.kind == "svc_off":
        return all(_unit_state(u) not in ("enabled", "enabled-runtime", "alias")
                   for u in t.data)
    if t.kind == "svc_on":
        return all(_unit_state(u) == "enabled" for u in t.data if _unit_exists(u))
    if t.kind == "dropin":
        try:
            with open(t.data[0]) as f:
                return f.read() == t.data[1]
        except OSError:
            return False
    if t.kind == "cmd":
        code, _ = run(["sh", "-c", t.data[0]], timeout=15)
        return code == 0
    return False


def user_tweak_steps(t, apply):
    """argv lists for a user-level tweak (no root)."""
    if t.kind == "autostart":
        return autostart_steps(t.data, enable=not apply)
    if t.kind == "gset":
        if apply:
            return [["gsettings", "set", s, k, v] for s, k, v in t.data]
        return [["gsettings", "reset", s, k] for s, k, _ in t.data]
    if t.kind == "umask":
        if apply:
            return [["systemctl", "--user", "mask", "--now", t.data]]
        return [["systemctl", "--user", "unmask", t.data]]
    raise ValueError(t.id)


def root_tweak_script(t, apply):
    """Shell lines the helper runs as root for a root tweak."""
    if t.kind in ("svc_off", "svc_on"):
        units = [u for u in t.data if _unit_exists(u)]
        if not units:
            return []
        on = (t.kind == "svc_on") == apply
        verb = "enable --now" if on else "disable --now"
        return [f"systemctl {verb} {' '.join(units)}"]
    if t.kind == "dropin":
        path, content, reload_cmd = t.data
        lines = []
        if apply:
            lines.append(("write", path, content))
        else:
            lines.append(("remove", path))
        if reload_cmd:
            lines.append(reload_cmd)
        return lines
    if t.kind == "cmd":
        return [t.data[1] if apply else t.data[2]]
    raise ValueError(t.id)


# --------------------------------------------------------------------------
# Cleaner
# --------------------------------------------------------------------------
@dataclass
class Clean:
    id: str
    title: str
    desc: str
    root: bool
    default: bool = True
    icon: str = "user-trash-symbolic"
    deep: bool = False


CLEANS = [
    Clean("apt-cache", "Package cache", "Downloaded .deb files in /var/cache/apt",
          True, icon="package-x-generic-symbolic"),
    Clean("autoremove", "Old kernels and unused packages",
          "Packages nothing needs any more (apt autoremove)", True,
          icon="application-x-firmware-symbolic"),
    Clean("snap-revisions", "Old snap versions", "Disabled snap revisions kept for rollback",
          True, icon="view-paged-symbolic"),
    Clean("journal", "System logs", "Journal entries above 100 MB", True,
          icon="text-x-generic-symbolic"),
    Clean("crash", "Crash reports", "Files in /var/crash", True,
          icon="dialog-warning-symbolic"),
    Clean("thumbnails", "Thumbnail cache", "Preview pictures in ~/.cache/thumbnails",
          False, icon="image-x-generic-symbolic"),
    Clean("trash", "Trash", "Files you moved to the Trash", False),
    Clean("user-temp", "Old temp files", "Your files in /tmp older than 7 days", False,
          icon="folder-symbolic"),
    Clean("browser-cache", "Browser caches",
          "Cache folders of Firefox, Chrome, Chromium, Brave, Opera, Edge. Close browsers first.",
          False, default=False, icon="web-browser-symbolic"),
    Clean("flatpak-unused", "Unused Flatpak runtimes",
          "Runtimes no installed app uses", False, icon="application-x-addon-symbolic"),
    # Deep clean: never selected by default
    Clean("app-caches", "All app caches",
          "Everything in ~/.cache (except AI models and browser engines). Close apps first.",
          False, default=False, icon="folder-symbolic", deep=True),
    Clean("sandbox-caches", "Snap and Flatpak app caches",
          "Cache folders inside ~/snap and ~/.var/app", False, default=False,
          icon="application-x-addon-symbolic", deep=True),
    Clean("dev-caches", "Developer caches",
          "pip, uv, npm, yarn, Cargo, Go and Gradle downloads. Builds download them again (needs internet).",
          False, default=False, icon="text-x-generic-symbolic", deep=True),
    Clean("orphan-configs", "Leftovers of removed apps",
          "Settings folders of apps, snaps and Flatpaks that are no longer installed. Moved to the Trash.",
          False, default=False, icon="edit-delete-symbolic", deep=True),
    Clean("old-logs", "Old rotated logs", "Compressed and numbered log files in /var/log",
          True, default=False, icon="text-x-generic-symbolic", deep=True),
    Clean("coredumps", "Core dumps", "Program crash dumps in /var/lib/systemd/coredump",
          True, default=False, icon="dialog-warning-symbolic", deep=True),
    Clean("rc-packages", "Settings of removed packages",
          "Config files that removed packages left behind (dpkg rc state)",
          True, default=False, icon="package-x-generic-symbolic", deep=True),
    Clean("old-kernels", "Old kernels",
          "Keeps the running kernel and one fallback, removes the rest",
          True, default=False, icon="application-x-firmware-symbolic", deep=True),
    Clean("locales", "Unused language packs",
          "Language packs, spell checkers and translations for languages this PC does not use",
          True, default=False, icon="font-x-generic-symbolic", deep=True),
]
CLEAN_BY_ID = {c.id: c for c in CLEANS}

HOME = os.path.expanduser("~")
BROWSER_CACHES = [
    "~/.cache/mozilla", "~/.cache/google-chrome", "~/.cache/chromium",
    "~/.cache/BraveSoftware", "~/.cache/opera", "~/.cache/microsoft-edge",
    "~/snap/firefox/common/.cache", "~/snap/chromium/common/.cache",
    "~/snap/brave/common/.cache", "~/snap/opera/common/.cache",
    "~/.var/app/org.mozilla.firefox/cache", "~/.var/app/com.brave.Browser/cache",
]


def du(path):
    """Bytes used by path (files counted once, symlinks not followed)."""
    total = 0
    try:
        st = os.lstat(path)
    except OSError:
        return 0
    if not os.path.isdir(path) or os.path.islink(path):
        return st.st_blocks * 512
    for root, dirs, files in os.walk(path, onerror=lambda e: None):
        for n in files:
            try:
                total += os.lstat(os.path.join(root, n)).st_blocks * 512
            except OSError:
                pass
    return total


def parse_size(s):
    m = re.match(r"([\d.]+)\s*([KMGT]?)i?B?", s.strip())
    if not m:
        return 0
    mult = {"": 1, "K": 1024, "M": 1024 ** 2, "G": 1024 ** 3, "T": 1024 ** 4}[m.group(2)]
    return int(float(m.group(1)) * mult)


def parse_autoremove(text):
    """Packages an `apt-get -s` run would remove or purge."""
    return [m.group(1) for m in re.finditer(r"^(?:Remv|Purg) (\S+)", text, re.M)]


# ---- deep clean helpers ----------------------------------------------------
CACHE_KEEP = {"thumbnails", "huggingface", "torch", "ms-playwright", "whisper",
              "glint", "tracker3", "localsearch"}
DEV_CACHES = ["~/.cache/pip", "~/.cache/uv", "~/.cache/go-build", "~/.cache/yarn",
              "~/.cache/pnpm", "~/.npm/_cacache", "~/.cargo/registry/cache", "~/.cargo/git/db",
              "~/.gradle/caches", "~/.gradle/wrapper/dists", "~/.local/share/pnpm/store"]


def app_cache_paths():
    base = os.path.join(HOME, ".cache")
    try:
        names = os.listdir(base)
    except OSError:
        return []
    return [os.path.join(base, n) for n in sorted(names) if n not in CACHE_KEEP]


def sandbox_cache_paths():
    pats = ["~/snap/*/common/.cache", "~/snap/*/[0-9]*/.cache", "~/.var/app/*/cache"]
    res = []
    for p in pats:
        res += glob.glob(os.path.expanduser(p))
    return sorted(set(res))


def dev_cache_paths():
    return [p for p in (os.path.expanduser(x) for x in DEV_CACHES) if os.path.exists(p)]


# (folder, indicators): the folder is a leftover when no indicator is present.
# Indicators: "cmd:NAME" on PATH, "snap:NAME", "flatpak:APP-ID".
ORPHAN_DIRS = [
    ("~/.config/transmission", ["cmd:transmission-gtk", "flatpak:com.transmissionbt.Transmission"]),
    ("~/.local/share/rhythmbox", ["cmd:rhythmbox"]),
    ("~/.local/share/shotwell", ["cmd:shotwell"]),
    ("~/.config/remmina", ["cmd:remmina", "flatpak:org.remmina.Remmina"]),
    ("~/.config/libreoffice", ["cmd:libreoffice", "cmd:soffice", "snap:libreoffice"]),
    ("~/.thunderbird", ["cmd:thunderbird", "snap:thunderbird"]),
    ("~/.mozilla", ["cmd:firefox", "snap:firefox", "cmd:librewolf"]),
    ("~/.config/google-chrome", ["cmd:google-chrome", "cmd:google-chrome-stable"]),
    ("~/.config/chromium", ["cmd:chromium", "cmd:chromium-browser", "snap:chromium"]),
    ("~/.config/BraveSoftware", ["cmd:brave-browser", "cmd:brave", "snap:brave"]),
    ("~/.config/opera", ["cmd:opera", "snap:opera"]),
    ("~/.config/microsoft-edge", ["cmd:microsoft-edge", "cmd:microsoft-edge-stable"]),
    ("~/.config/vivaldi", ["cmd:vivaldi", "cmd:vivaldi-stable"]),
    ("~/.config/Code", ["cmd:code", "snap:code"]),
    ("~/.config/VSCodium", ["cmd:codium"]),
    ("~/.config/discord", ["cmd:discord", "snap:discord"]),
    ("~/.config/Slack", ["cmd:slack", "snap:slack"]),
    ("~/.config/spotify", ["cmd:spotify", "snap:spotify"]),
    ("~/.config/obs-studio", ["cmd:obs"]),
    ("~/.zoom", ["cmd:zoom"]),
    ("~/.config/teams-for-linux", ["cmd:teams-for-linux", "snap:teams-for-linux"]),
    ("~/.config/skypeforlinux", ["cmd:skypeforlinux", "snap:skype"]),
    ("~/.config/GIMP", ["cmd:gimp"]),
    ("~/.config/inkscape", ["cmd:inkscape"]),
    ("~/.config/audacity", ["cmd:audacity"]),
    ("~/.config/vlc", ["cmd:vlc"]),
    ("~/.config/Element", ["cmd:element-desktop"]),
    ("~/.config/Signal", ["cmd:signal-desktop"]),
    ("~/.local/share/TelegramDesktop", ["cmd:telegram-desktop", "cmd:Telegram", "snap:telegram-desktop"]),
]


def orphan_paths(snaps=None, flats=None):
    snaps = installed_snaps() if snaps is None else snaps
    flats = installed_flatpaks() if flats is None else flats

    def present(ind):
        kind, _, name = ind.partition(":")
        return (kind == "cmd" and have(name)) or (kind == "snap" and name in snaps) or \
            (kind == "flatpak" and name in flats)
    res = []
    for path, inds in ORPHAN_DIRS:
        p = os.path.expanduser(path)
        if os.path.isdir(p) and not os.path.islink(p) and not any(present(i) for i in inds):
            res.append(p)
    # data of Flatpaks and snaps that are gone
    var = os.path.join(HOME, ".var/app")
    if os.path.isdir(var) and have("flatpak"):
        for n in sorted(os.listdir(var)):
            if n not in flats and FLATPAK_RE.match(n):
                res.append(os.path.join(var, n))
    sdir = os.path.join(HOME, "snap")
    if os.path.isdir(sdir) and have("snap") and snaps:
        for n in sorted(os.listdir(sdir)):
            if n not in snaps and PKG_RE.match(n):
                res.append(os.path.join(sdir, n))
    return res


OLD_LOG_RE = re.compile(r"\.(gz|xz|bz2|zst|old)$|\.\d+$")


def old_log_files(root="/var/log"):
    res = []
    for d, dirs, files in os.walk(root, onerror=lambda e: None):
        if "journal" in dirs:
            dirs.remove("journal")
        for n in files:
            if OLD_LOG_RE.search(n):
                p = os.path.join(d, n)
                if os.path.isfile(p) and not os.path.islink(p):
                    res.append(p)
    return res


KERNEL_RE = re.compile(r"^linux-(?:image|image-unsigned|modules|modules-extra|headers|tools|"
                       r"cloud-tools|buildinfo|objects|signatures)-(\d+\.\d+\.\d+-\d+)(?:-[a-z0-9]+)?$")


def kernel_version(release):
    m = re.match(r"(\d+\.\d+\.\d+-\d+)", release or "")
    return m.group(1) if m else None


def kernel_pkg_version(pkg):
    m = KERNEL_RE.match(pkg)
    return m.group(1) if m else None


def _vkey(v):
    return tuple(int(x) for x in re.split(r"[.-]", v))


def old_kernel_pkgs(pkgs, running):
    """Kernel packages to purge: keep the running kernel and one fallback."""
    run_v = kernel_version(running)
    vers = sorted({v for v in map(kernel_pkg_version, pkgs) if v}, key=_vkey, reverse=True)
    if not run_v or run_v not in vers:
        return []
    keep = {run_v}
    for v in vers:
        if v != run_v:
            keep.add(v)
            break
    return sorted(p for p in pkgs if kernel_pkg_version(p) and kernel_pkg_version(p) not in keep)


LOCALE_PKG_RE = re.compile(
    r"^(?:language-pack-gnome|language-pack|hunspell|hyphen|mythes|libreoffice-l10n|"
    r"libreoffice-help|firefox-locale|thunderbird-locale|aspell|myspell|gimp-help)"
    r"-([a-z]{2,3})(?:-[a-z0-9-]+)?$")
NOT_LANG = {"doc", "dev", "dbg", "all", "gtk", "kde", "bin", "gnu"}


def lang_codes(text):
    codes = set()
    for tok in re.split(r"[=:\s\"',]+", text):
        m = re.match(r"^([a-z]{2,3})(?:_|\.|@|$)", tok)
        if m and tok not in ("C",):
            codes.add(m.group(1))
    return codes


def used_langs():
    text = read("/etc/default/locale") + "\n" + read("/etc/locale.conf")
    code, out = run(["locale", "-a"], timeout=10)
    return lang_codes(text) | lang_codes(out) | {"en"}


def locale_pkgs_to_remove(pkgs, langs):
    res = []
    for p in pkgs:
        m = LOCALE_PKG_RE.match(p)
        if m and m.group(1) not in NOT_LANG and m.group(1) not in langs:
            res.append(p)
    return sorted(res)


def installed_pkgs(pattern_list):
    code, out = run(["dpkg-query", "-W", "-f=${Package} ${db:Status-Abbrev}\n"] + pattern_list,
                    timeout=30)
    return sorted({l.split()[0] for l in out.splitlines()
                   if len(l.split()) >= 2 and l.split()[1].startswith("ii")})


LOCALE_GLOBS = ["language-pack-*", "hunspell-*", "hyphen-*", "mythes-*", "libreoffice-l10n-*",
                "libreoffice-help-*", "firefox-locale-*", "thunderbird-locale-*", "aspell-*",
                "myspell-*", "gimp-help-*"]
KERNEL_GLOBS = ["linux-image-[0-9]*", "linux-image-unsigned-[0-9]*", "linux-modules-[0-9]*",
                "linux-modules-extra-[0-9]*", "linux-headers-[0-9]*", "linux-tools-[0-9]*",
                "linux-cloud-tools-[0-9]*", "linux-buildinfo-[0-9]*"]


def rc_packages():
    code, out = run(["dpkg-query", "-W", "-f=${Package} ${db:Status-Abbrev}\n"], timeout=30)
    return sorted({l.split()[0] for l in out.splitlines()
                   if len(l.split()) >= 2 and l.split()[1].startswith("rc")})


def pkgs_size(pkgs):
    if not pkgs:
        return 0
    code, out = run(["dpkg-query", "-W", "-f=${Installed-Size}\n"] + list(pkgs), timeout=30)
    return sum(int(x) for x in out.split() if x.isdigit()) * 1024


def _rm_path(p):
    try:
        if os.path.isdir(p) and not os.path.islink(p):
            shutil.rmtree(p, ignore_errors=True)
        else:
            os.unlink(p)
        return True
    except OSError:
        return False


def parse_snap_disabled(text):
    out = []
    for line in text.splitlines()[1:]:
        cols = line.split()
        if len(cols) >= 3 and "disabled" in line.split()[-1].split(","):
            out.append((cols[0], cols[2]))
    return out


def user_temp_files(days=7):
    cutoff = time.time() - days * 86400
    uid = os.getuid()
    found = []
    try:
        entries = os.scandir("/tmp")
    except OSError:
        return found
    for e in entries:
        try:
            st = e.stat(follow_symlinks=False)
        except OSError:
            continue
        if st.st_uid == uid and st.st_mtime < cutoff and not e.name.startswith((".X", ".ICE", "systemd-")):
            found.append(e.path)
    return found


def clean_size(cid):
    """Reclaimable bytes for a cleaner category, or None if unknown."""
    if cid == "apt-cache":
        return sum(du(p) for p in glob.glob("/var/cache/apt/archives/*.deb")) + \
            du("/var/cache/apt/archives/partial") + \
            sum(du(p) for p in glob.glob("/var/cache/apt/*.bin"))
    if cid == "autoremove":
        code, out = run(["apt-get", "-s", "autoremove"], timeout=60)
        pkgs = parse_autoremove(out)
        if not pkgs:
            return 0
        code, out = run(["dpkg-query", "-W", "-f=${Installed-Size}\n"] + pkgs, timeout=30)
        return sum(int(x) for x in out.split() if x.isdigit()) * 1024
    if cid == "snap-revisions":
        if not have("snap"):
            return 0
        code, out = run(["snap", "list", "--all"], timeout=30)
        return sum(du(f"/var/lib/snapd/snaps/{n}_{r}.snap") for n, r in parse_snap_disabled(out))
    if cid == "journal":
        code, out = run(["journalctl", "--disk-usage"], timeout=30)
        m = re.search(r"take up ([\d.]+\s*[KMGT]?)", out)
        size = parse_size(m.group(1)) if m else 0
        return max(0, size - 100 * 1024 ** 2)
    if cid == "crash":
        return sum(du(p) for p in glob.glob("/var/crash/*"))
    if cid == "thumbnails":
        return du(os.path.join(HOME, ".cache/thumbnails"))
    if cid == "trash":
        return du(os.path.join(HOME, ".local/share/Trash/files"))
    if cid == "user-temp":
        return sum(du(p) for p in user_temp_files())
    if cid == "browser-cache":
        return sum(du(os.path.expanduser(p)) for p in BROWSER_CACHES)
    if cid == "flatpak-unused":
        return None
    if cid == "app-caches":
        return sum(du(p) for p in app_cache_paths())
    if cid == "sandbox-caches":
        return sum(du(p) for p in sandbox_cache_paths())
    if cid == "dev-caches":
        return sum(du(p) for p in dev_cache_paths())
    if cid == "orphan-configs":
        return sum(du(p) for p in orphan_paths())
    if cid == "old-logs":
        return sum(du(p) for p in old_log_files())
    if cid == "coredumps":
        return du("/var/lib/systemd/coredump")
    if cid == "rc-packages":
        return None if rc_packages() else 0
    if cid == "old-kernels":
        return pkgs_size(old_kernel_pkgs(installed_pkgs(KERNEL_GLOBS), os.uname().release))
    if cid == "locales":
        return pkgs_size(locale_pkgs_to_remove(installed_pkgs(LOCALE_GLOBS), used_langs()))
    return 0


def _rm_children(path):
    """Remove everything inside path without following symlinks out of it."""
    if not os.path.isdir(path) or os.path.islink(path):
        return
    for e in os.scandir(path):
        try:
            if e.is_dir(follow_symlinks=False):
                shutil.rmtree(e.path, ignore_errors=True)
            else:
                os.unlink(e.path)
        except OSError:
            pass


def user_clean(cid, log=print):
    """Clean a user-level category in-process. Returns True on success."""
    if cid == "thumbnails":
        _rm_children(os.path.join(HOME, ".cache/thumbnails"))
    elif cid == "trash":
        code, out = run(["gio", "trash", "--empty"], timeout=300)
        log(out.strip() or "Trash emptied")
        return code == 0
    elif cid == "user-temp":
        for p in user_temp_files():
            try:
                if os.path.isdir(p) and not os.path.islink(p):
                    shutil.rmtree(p, ignore_errors=True)
                else:
                    os.unlink(p)
                log(f"removed {p}")
            except OSError as e:
                log(f"skipped {p}: {e.strerror}")
    elif cid == "browser-cache":
        for p in BROWSER_CACHES:
            p = os.path.expanduser(p)
            if os.path.isdir(p):
                _rm_children(p)
                log(f"emptied {p}")
    elif cid in ("app-caches", "dev-caches"):
        for p in (app_cache_paths() if cid == "app-caches" else dev_cache_paths()):
            log(("removed " if _rm_path(p) else "skipped ") + p)
    elif cid == "sandbox-caches":
        for p in sandbox_cache_paths():
            if os.path.isdir(p) and not os.path.islink(p):
                _rm_children(p)
                log(f"emptied {p}")
    elif cid == "orphan-configs":
        paths = orphan_paths()
        if not paths:
            log("no leftovers found")
        for p in paths:
            code, out = run(["gio", "trash", "--", p], timeout=120)
            log((f"moved to Trash: {p}" if code == 0 else f"skipped {p}: {out.strip()}"))
    else:
        raise ValueError(cid)
    return True


def user_clean_argv(cid):
    if cid == "flatpak-unused":
        return ["flatpak", "uninstall", "--unused", "-y", "--noninteractive"]
    return None


ROOT_CLEAN = {
    "apt-cache": ["apt-get clean"],
    "autoremove": ["DEBIAN_FRONTEND=noninteractive apt-get -y autoremove --purge"],
    "journal": ["journalctl --vacuum-size=100M"],
    "crash": [("crash",)],
    "snap-revisions": [("snaps",)],
    "old-logs": [("oldlogs",)],
    "coredumps": [("coredumps",)],
    "rc-packages": [("rc",)],
    "old-kernels": [("kernels",)],
    "locales": [("locales",)],
}


# --------------------------------------------------------------------------
# Utilities
# --------------------------------------------------------------------------
@dataclass
class Util:
    id: str
    group: str
    title: str
    desc: str
    root: bool
    script: list = field(default_factory=list)
    needs: str = ""
    icon: str = "applications-system-symbolic"


UTIL_GROUPS = [("maintain", "Maintenance"), ("repair", "Repair"), ("diagnose", "Diagnose")]

UTILS = [
    Util("update-all", "maintain", "Update everything",
         "apt full-upgrade, then snap refresh", True,
         ["apt-get update", "DEBIAN_FRONTEND=noninteractive apt-get -y full-upgrade",
          "snap refresh || true"], icon="software-update-available-symbolic"),
    Util("flatpak-update", "maintain", "Update Flatpak apps",
         "flatpak update for your user and the system", False,
         [["flatpak", "update", "-y", "--noninteractive"]], needs="flatpak",
         icon="system-software-install-symbolic"),
    Util("trim", "maintain", "TRIM SSDs now",
         "Tell SSDs which blocks are free (fstrim)", True, ["fstrim -av"],
         icon="drive-harddisk-solidstate-symbolic"),
    Util("drop-caches", "maintain", "Free cached memory",
         "Write pending data, then drop page cache", True,
         ["sync", "sysctl vm.drop_caches=3", "free -h"], icon="edit-clear-all-symbolic"),
    Util("font-cache", "maintain", "Rebuild font cache",
         "Fixes missing or wrong fonts", False, [["fc-cache", "-f"]],
         icon="font-x-generic-symbolic"),
    Util("fix-packages", "repair", "Fix broken packages",
         "Finish interrupted installs and fix dependencies", True,
         ["dpkg --configure -a", "DEBIAN_FRONTEND=noninteractive apt-get -y -f install"],
         icon="emblem-system-symbolic"),
    Util("restart-network", "repair", "Restart network",
         "Turns networking off and on again", False,
         [["nmcli", "networking", "off"], ["sleep", "2"], ["nmcli", "networking", "on"]],
         needs="nmcli", icon="network-wireless-symbolic"),
    Util("restart-audio", "repair", "Restart audio",
         "Restarts PipeWire and WirePlumber", False,
         [["systemctl", "--user", "restart", "pipewire", "pipewire-pulse", "wireplumber"]],
         icon="audio-speakers-symbolic"),
    Util("flatpak-repair", "repair", "Repair Flatpak",
         "Checks and fixes your Flatpak installation", False,
         [["flatpak", "repair", "--user"]], needs="flatpak",
         icon="application-x-addon-symbolic"),
    Util("smart", "diagnose", "Disk health (SMART)",
         "Asks each disk if it is healthy", True, [("smart",)], needs="smartctl",
         icon="drive-harddisk-symbolic"),
    Util("failed", "diagnose", "Failed services",
         "Lists services that failed to start", False,
         [["systemctl", "--failed", "--no-pager"], ["systemctl", "--user", "--failed", "--no-pager"]],
         icon="dialog-error-symbolic"),
    Util("boot-time", "diagnose", "Boot time",
         "What made boot slow", False,
         [["systemd-analyze"], ["sh", "-c", "systemd-analyze blame --no-pager | head -15"]],
         icon="alarm-symbolic"),
    Util("errors", "diagnose", "Errors since boot",
         "The last 40 error messages from this boot", False,
         [["journalctl", "-b", "-p", "err", "-n", "40", "--no-pager"]],
         icon="dialog-information-symbolic"),
]
UTIL_BY_ID = {u.id: u for u in UTILS}


# --------------------------------------------------------------------------
# DNS
# --------------------------------------------------------------------------
DNS_PRESETS = [
    ("auto", "Automatic", "From your router or provider", []),
    ("cloudflare", "Cloudflare", "Fast, privacy focused", ["1.1.1.1", "1.0.0.1"]),
    ("google", "Google", "Reliable public DNS", ["8.8.8.8", "8.8.4.4"]),
    ("quad9", "Quad9", "Blocks malware domains", ["9.9.9.9", "149.112.112.112"]),
    ("adguard", "AdGuard", "Blocks ads and trackers", ["94.140.14.14", "94.140.15.15"]),
    ("opendns", "OpenDNS", "Cisco, optional filtering", ["208.67.222.222", "208.67.220.220"]),
]
IPV4_RE = re.compile(r"^(25[0-5]|2[0-4]\d|1?\d?\d)(\.(25[0-5]|2[0-4]\d|1?\d?\d)){3}$")


def dns_query_packet(name, qid=0x4747):
    q = b"".join(bytes([len(p)]) + p.encode() for p in name.split(".")) + b"\0"
    return struct.pack(">HHHHHH", qid, 0x0100, 1, 0, 0, 0) + q + struct.pack(">HH", 1, 1)


def dns_response_ok(data, qid=0x4747):
    if len(data) < 12:
        return False
    rid, flags, qd, an = struct.unpack(">HHHH", data[:8])
    return rid == qid and flags & 0x8000 and (flags & 0x000F) == 0 and an > 0


def dns_latency(server, name="example.com", tries=3, timeout=1.5):
    """Median reply time in ms, or None if the server did not answer."""
    times = []
    for i in range(tries):
        qid = 0x4700 + i
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.settimeout(timeout)
        try:
            t0 = time.perf_counter()
            s.sendto(dns_query_packet(name, qid), (server, 53))
            data, _ = s.recvfrom(512)
            if dns_response_ok(data, qid):
                times.append((time.perf_counter() - t0) * 1000)
        except OSError:
            pass
        finally:
            s.close()
    if not times:
        return None
    times.sort()
    return times[len(times) // 2]


def active_connection():
    """(uuid, name, device) of the main active NetworkManager connection."""
    code, out = run(["nmcli", "-t", "-f", "UUID,NAME,TYPE,DEVICE", "connection", "show", "--active"])
    if code != 0:
        return None
    best = None
    for line in out.splitlines():
        parts = re.split(r"(?<!\\):", line)
        if len(parts) < 4 or parts[2] in ("loopback", "bridge", "tun") or not parts[3]:
            continue
        cand = (parts[0], parts[1].replace("\\:", ":"), parts[3])
        if parts[2] in ("802-11-wireless", "802-3-ethernet") and best is None:
            best = cand
        elif best is None:
            best = cand
    return best


def current_dns():
    code, out = run(["resolvectl", "dns"], timeout=10)
    servers = []
    for line in out.splitlines():
        if ":" in line:
            servers += [s for s in line.split(":", 1)[1].split() if s]
    seen = []
    for s in servers:
        if s not in seen:
            seen.append(s)
    return seen


def dns_set_argv(uuid, servers):
    if servers:
        mod = ["nmcli", "connection", "modify", uuid, "ipv4.dns", " ".join(servers),
               "ipv4.ignore-auto-dns", "yes"]
    else:
        mod = ["nmcli", "connection", "modify", uuid, "ipv4.dns", "",
               "ipv4.ignore-auto-dns", "no"]
    return [mod, ["nmcli", "connection", "up", uuid]]


# --------------------------------------------------------------------------
# Apps and debloat
# --------------------------------------------------------------------------
@dataclass
class App:
    name: str
    cat: str
    kind: str       # apt | flatpak | snap
    pid: str
    desc: str

    @property
    def key(self):
        return f"{self.kind}:{self.pid}"


APP_CATS = ["Browsers", "Communication", "Media", "Graphics", "Office", "Development",
            "Utilities", "Gaming", "Security"]

_A = App
APPS = [
    _A("Firefox", "Browsers", "snap", "firefox", "Mozilla's web browser"),
    _A("Brave", "Browsers", "flatpak", "com.brave.Browser", "Browser with built-in ad blocking"),
    _A("Chromium", "Browsers", "snap", "chromium", "Open source base of Chrome"),
    _A("LibreWolf", "Browsers", "flatpak", "io.gitlab.librewolf-community", "Privacy-hardened Firefox"),
    _A("Vivaldi", "Browsers", "flatpak", "com.vivaldi.Vivaldi", "Feature-rich browser"),
    _A("Tor Browser", "Browsers", "flatpak", "org.torproject.torbrowser-launcher", "Anonymous browsing"),
    _A("Thunderbird", "Communication", "snap", "thunderbird", "Email and calendar"),
    _A("Telegram", "Communication", "flatpak", "org.telegram.desktop", "Messenger"),
    _A("Signal", "Communication", "flatpak", "org.signal.Signal", "Private messenger"),
    _A("Discord", "Communication", "flatpak", "com.discordapp.Discord", "Voice and text chat"),
    _A("Zoom", "Communication", "flatpak", "us.zoom.Zoom", "Video meetings"),
    _A("Element", "Communication", "flatpak", "im.riot.Riot", "Matrix chat"),
    _A("VLC", "Media", "apt", "vlc", "Plays every video and audio format"),
    _A("mpv", "Media", "apt", "mpv", "Minimal video player"),
    _A("OBS Studio", "Media", "flatpak", "com.obsproject.Studio", "Screen recording and streaming"),
    _A("Audacity", "Media", "flatpak", "org.audacityteam.Audacity", "Audio editor"),
    _A("Kdenlive", "Media", "flatpak", "org.kde.kdenlive", "Video editor"),
    _A("Shotcut", "Media", "flatpak", "org.shotcut.Shotcut", "Video editor"),
    _A("HandBrake", "Media", "flatpak", "fr.handbrake.ghb", "Video converter"),
    _A("Spotify", "Media", "snap", "spotify", "Music streaming"),
    _A("Strawberry", "Media", "flatpak", "org.strawberrymusicplayer.strawberry", "Music library player"),
    _A("GIMP", "Graphics", "flatpak", "org.gimp.GIMP", "Image editor"),
    _A("Inkscape", "Graphics", "flatpak", "org.inkscape.Inkscape", "Vector graphics"),
    _A("Krita", "Graphics", "flatpak", "org.kde.krita", "Digital painting"),
    _A("Blender", "Graphics", "flatpak", "org.blender.Blender", "3D creation"),
    _A("darktable", "Graphics", "flatpak", "org.darktable.Darktable", "RAW photo editor"),
    _A("Pinta", "Graphics", "flatpak", "com.github.PintaProject.Pinta", "Simple painting"),
    _A("LibreOffice", "Office", "apt", "libreoffice", "Office suite"),
    _A("OnlyOffice", "Office", "flatpak", "org.onlyoffice.desktopeditors", "MS Office compatible suite"),
    _A("Obsidian", "Office", "flatpak", "md.obsidian.Obsidian", "Markdown notes"),
    _A("Zotero", "Office", "flatpak", "org.zotero.Zotero", "Reference manager"),
    _A("Okular", "Office", "flatpak", "org.kde.okular", "PDF viewer and annotator"),
    _A("Xournal++", "Office", "flatpak", "com.github.xournalpp.xournalpp", "Handwritten notes and PDF"),
    _A("VS Code", "Development", "snap", "code", "Code editor"),
    _A("VSCodium", "Development", "flatpak", "com.vscodium.codium", "VS Code without telemetry"),
    _A("Git", "Development", "apt", "git", "Version control"),
    _A("Build tools", "Development", "apt", "build-essential", "gcc, make and friends"),
    _A("Python venv", "Development", "apt", "python3-venv", "Python virtual environments"),
    _A("Docker", "Development", "apt", "docker.io", "Containers"),
    _A("Neovim", "Development", "apt", "neovim", "Terminal editor"),
    _A("DBeaver", "Development", "flatpak", "io.dbeaver.DBeaverCommunity", "Database tool"),
    _A("Postman", "Development", "flatpak", "com.getpostman.Postman", "API testing"),
    _A("GNOME Tweaks", "Utilities", "apt", "gnome-tweaks", "More GNOME settings"),
    _A("Extension Manager", "Utilities", "flatpak", "com.mattjakeman.ExtensionManager", "Browse GNOME extensions"),
    _A("Flatseal", "Utilities", "flatpak", "com.github.tchx84.Flatseal", "Flatpak permissions"),
    _A("Timeshift", "Utilities", "apt", "timeshift", "System snapshots"),
    _A("GParted", "Utilities", "apt", "gparted", "Partition editor"),
    _A("htop", "Utilities", "apt", "htop", "Process viewer"),
    _A("btop", "Utilities", "apt", "btop", "Pretty resource monitor"),
    _A("Mission Center", "Utilities", "flatpak", "io.missioncenter.MissionCenter", "Task manager"),
    _A("SMART tools", "Utilities", "apt", "smartmontools", "Disk health checks"),
    _A("7-Zip", "Utilities", "apt", "7zip", "Archive tool"),
    _A("qBittorrent", "Utilities", "flatpak", "org.qbittorrent.qBittorrent", "Torrent client"),
    _A("Bottles", "Utilities", "flatpak", "com.usebottles.bottles", "Run Windows apps"),
    _A("Steam", "Gaming", "apt", "steam-installer", "Game store"),
    _A("Lutris", "Gaming", "flatpak", "net.lutris.Lutris", "Game launcher"),
    _A("Heroic", "Gaming", "flatpak", "com.heroicgameslauncher.hgl", "Epic and GOG games"),
    _A("ProtonUp-Qt", "Gaming", "flatpak", "net.davidotek.pupgui2", "Proton versions"),
    _A("MangoHud", "Gaming", "apt", "mangohud", "FPS overlay"),
    _A("GameMode", "Gaming", "apt", "gamemode", "Gaming performance mode"),
    _A("KeePassXC", "Security", "flatpak", "org.keepassxc.KeePassXC", "Password manager"),
    _A("Bitwarden", "Security", "flatpak", "com.bitwarden.desktop", "Password manager"),
    _A("ClamTk", "Security", "apt", "clamtk", "Virus scanner"),
    _A("Firewall (gufw)", "Security", "apt", "gufw", "Firewall settings"),
    _A("Proton VPN", "Security", "flatpak", "com.protonvpn.www", "VPN"),
]
APP_BY_KEY = {a.key: a for a in APPS}


@dataclass
class Bloat:
    id: str
    title: str
    desc: str
    apt: list
    snap: list = field(default_factory=list)
    rec: bool = False
    deep: bool = False
    warn: str = ""


BLOAT = [
    Bloat("games", "GNOME games", "Solitaire, Mahjongg, Mines, Sudoku",
          ["aisleriot", "gnome-mahjongg", "gnome-mines", "gnome-sudoku"], rec=True),
    Bloat("rhythmbox", "Rhythmbox", "Music player", ["rhythmbox"]),
    Bloat("shotwell", "Shotwell", "Photo manager", ["shotwell"]),
    Bloat("remmina", "Remmina", "Remote desktop client", ["remmina"]),
    Bloat("transmission", "Transmission", "Torrent client", ["transmission-gtk"]),
    Bloat("totem", "Videos (Totem)", "Video player", ["totem"]),
    Bloat("deja-dup", "Backups (Déjà Dup)", "Backup tool", ["deja-dup"]),
    Bloat("thunderbird", "Thunderbird", "Email client", ["thunderbird"], ["thunderbird"]),
    Bloat("libreoffice", "LibreOffice", "Office suite (about 500 MB)",
          ["libreoffice-core", "libreoffice-common"]),
    Bloat("simple-scan", "Document Scanner", "Scanner app", ["simple-scan"]),
    Bloat("snapshot", "Camera (Snapshot)", "Webcam app", ["gnome-snapshot", "cheese"]),
    Bloat("maps", "Maps", "GNOME Maps", ["gnome-maps"]),
    Bloat("weather", "Weather", "GNOME Weather", ["gnome-weather"]),
    Bloat("contacts", "Contacts", "GNOME Contacts", ["gnome-contacts"]),
    Bloat("clocks", "Clocks", "GNOME Clocks", ["gnome-clocks"]),
    Bloat("calendar", "Calendar", "GNOME Calendar", ["gnome-calendar"]),
    Bloat("yelp", "Help", "Ubuntu desktop guide", ["yelp", "gnome-user-docs", "ubuntu-docs"]),
    Bloat("firefox", "Firefox", "Only if you use another browser", [], ["firefox"]),
    # Deep debloat
    Bloat("telemetry", "Usage reporting", "ubuntu-insights, ubuntu-report, popularity-contest",
          ["ubuntu-insights", "ubuntu-report", "popularity-contest"], rec=True, deep=True),
    Bloat("crash-tools", "Crash reporting", "Apport, Whoopsie and Kerneloops",
          ["apport", "apport-gtk", "apport-symptoms", "apport-core-dump-handler", "python3-apport",
           "whoopsie", "kerneloops"], rec=True, deep=True),
    Bloat("cloud-init", "cloud-init", "Cloud server setup tool; a desktop does not need it",
          ["cloud-init"], rec=True, deep=True),
    Bloat("pro", "Ubuntu Pro client", "Pro/ESM adverts and attach tool",
          ["ubuntu-pro-client", "ubuntu-pro-client-l10n", "ubuntu-advantage-tools"], deep=True,
          warn="You can no longer attach Ubuntu Pro (free ESM updates)."),
    Bloat("landscape", "Landscape", "Canonical fleet management client and login stats",
          ["landscape-common", "landscape-client"], rec=True, deep=True),
    Bloat("snap-store", "App Center (snap)", "Ubuntu's snap store app", [], ["snap-store"], deep=True,
          warn="Install apps from the Apps page or GNOME Software instead."),
    Bloat("remote", "Desktop sharing", "GNOME Remote Desktop and Rygel media sharing",
          ["gnome-remote-desktop", "rygel"], deep=True,
          warn="Settings > Sharing loses remote desktop and media sharing."),
    Bloat("a11y", "Screen reader and braille", "Orca, BRLTTY, speech-dispatcher",
          ["orca", "brltty", "speech-dispatcher"], deep=True,
          warn="Only if nobody on this PC needs a screen reader. BRLTTY also grabs some USB-serial boards."),
    Bloat("gnome-extras", "Extra GNOME tools", "Characters, Logs, Fonts",
          ["gnome-characters", "gnome-logs", "gnome-font-viewer"], deep=True),
    Bloat("cjk-fonts", "Chinese, Japanese, Korean fonts", "About 250 MB of Noto CJK fonts",
          ["fonts-noto-cjk", "fonts-noto-cjk-extra"], deep=True,
          warn="CJK text on web pages shows as boxes."),
]

# ---- removing snap completely ---------------------------------------------
SNAP_FLATPAK = {
    "firefox": "org.mozilla.firefox", "thunderbird": "org.mozilla.Thunderbird",
    "chromium": "org.chromium.Chromium", "brave": "com.brave.Browser",
    "opera": "com.opera.Opera", "xournalpp": "com.github.xournalpp.xournalpp",
    "spotify": "com.spotify.Client", "code": "com.visualstudio.code",
    "telegram-desktop": "org.telegram.desktop", "vlc": "org.videolan.VLC",
    "discord": "com.discordapp.Discord", "slack": "com.slack.Slack",
    "obs-studio": "com.obsproject.Studio", "gimp": "org.gimp.GIMP",
    "inkscape": "org.inkscape.Inkscape", "libreoffice": "org.libreoffice.LibreOffice",
    "zoom-client": "us.zoom.Zoom", "postman": "com.getpostman.Postman",
    "signal-desktop": "org.signal.Signal", "firmware-updater": "org.gnome.Firmware",
    "krita": "org.kde.krita", "blender": "org.blender.Blender",
}
# snap -> [(path under ~/snap/NAME, Flatpak destination)]
SNAP_PROFILES = {
    "firefox": [("common/.mozilla/firefox", "~/.var/app/org.mozilla.firefox/.mozilla/firefox")],
    "thunderbird": [("common/.thunderbird", "~/.var/app/org.mozilla.Thunderbird/.thunderbird")],
    "brave": [("current/.config/BraveSoftware", "~/.var/app/com.brave.Browser/config/BraveSoftware")],
    "opera": [("current/.config/opera", "~/.var/app/com.opera.Opera/config/opera")],
    "chromium": [("common/chromium", "~/.var/app/org.chromium.Chromium/config/chromium")],
    "xournalpp": [("current/.config/xournalpp",
                   "~/.var/app/com.github.xournalpp.xournalpp/config/xournalpp")],
    "telegram-desktop": [("current/.local/share/TelegramDesktop",
                          "~/.var/app/org.telegram.desktop/data/TelegramDesktop")],
}
NO_SNAP_PIN = "/etc/apt/preferences.d/glint-no-snap"
NO_SNAP_PIN_TEXT = ("# Glint: keeps snapd off this system. Delete this file (or use\n"
                    "# Glint > Debloat > Restore Snap) to allow snapd again.\n"
                    "Package: snapd\nPin: release a=*\nPin-Priority: -10\n")


def snap_installed():
    return have("snap") or bool(installed_apt(["snapd"]))


def snap_blocked():
    return os.path.exists(NO_SNAP_PIN)


def snap_replacements(snaps):
    """[(snap, flatpak id)] for installed app snaps with a Flathub version."""
    return [(s, SNAP_FLATPAK[s]) for s in sorted(snaps) if s in SNAP_FLATPAK]


def migrate_snap_profiles(log=print, snaps=None):
    """Copy browser/app profiles from ~/snap into the Flatpak folders (never overwrites)."""
    snaps = installed_snaps() if snaps is None else snaps
    for name, pairs in SNAP_PROFILES.items():
        if name not in snaps:
            continue
        for rel, dst in pairs:
            src = os.path.join(HOME, "snap", name, rel)
            dst = os.path.expanduser(dst)
            if not os.path.isdir(src):
                continue
            if os.path.exists(dst):
                log(f"kept existing {dst}")
                continue
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            shutil.copytree(src, dst, symlinks=True)
            log(f"copied {name} profile to {dst}")
    return True


def backup_snap_home(log=print):
    """Move ~/snap aside; `snap remove` would delete the data inside it."""
    src = os.path.join(HOME, "snap")
    if not os.path.isdir(src) or os.path.islink(src):
        log("no ~/snap folder")
        return True
    dst = os.path.join(data_dir(), "snap-backup-" + time.strftime("%Y%m%d-%H%M%S"))
    os.rename(src, dst)
    log(f"moved ~/snap to {dst}")
    return True


def snap_removal_order(snap_list_text):
    """Names from `snap list`: apps first, then content/theme snaps, bases, snapd last."""
    apps, bases, last = [], [], []
    for line in snap_list_text.splitlines()[1:]:
        cols = line.split()
        if not cols:
            continue
        notes = cols[-1].split(",")
        if cols[0] == "snapd" or "snapd" in notes:
            last.append(cols[0])
        elif "base" in notes or cols[0] in ("bare",) or re.match(r"core\d*$", cols[0]):
            bases.append(cols[0])
        else:
            apps.append(cols[0])
    content = [a for a in apps if re.match(r"(gnome-|gtk-|mesa-|kf\d|icon-theme|.*-themes?$)", a)]
    apps = [a for a in apps if a not in content]
    return apps + content + bases + last
BLOAT_BY_ID = {b.id: b for b in BLOAT}


def installed_apt(pkgs):
    if not pkgs:
        return set()
    code, out = run(["dpkg-query", "-W", "-f=${Package} ${db:Status-Abbrev}\n"] + list(pkgs),
                    timeout=30)
    res = set()
    for line in out.splitlines():
        parts = line.split()
        if len(parts) >= 2 and parts[1].startswith("ii"):
            res.add(parts[0])
    return res


SIM_ENV = {"PATH": "/usr/sbin:/usr/bin:/sbin:/bin", "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8"}


def apt_sim(args):
    """Simulate an apt-get action (works without root). -> (ok, [packages removed], output)"""
    code, out = run(["apt-get", "-s"] + list(args), timeout=180, env=SIM_ENV)
    return code == 0, parse_autoremove(out), out


def removal_plan(pkgs, purge):
    """What really goes when these packages are removed.

    -> (direct, orphans): direct = everything apt removes for the request (may include
    packages that depend on them); orphans = packages that only become unused because
    of this removal (pre-existing orphans are not touched).
    """
    pkgs = sorted(pkgs)
    if not pkgs:
        return [], []
    verb = "purge" if purge else "remove"
    ok, direct, out = apt_sim([verb] + pkgs)
    if not ok:
        raise RuntimeError(out.strip().splitlines()[-1] if out.strip() else "apt simulation failed")
    orphans = []
    if purge:
        ok2, combined, _ = apt_sim([verb, "--autoremove"] + pkgs)
        _ok3, before, _ = apt_sim(["autoremove"])
        if ok2:
            orphans = sorted(set(combined) - set(direct) - set(before))
    return sorted(direct), orphans


def bloat_plan(b, purge):
    return removal_plan(sorted(installed_apt(b.apt)), purge)


def clean_plan(cid):
    """Packages a root clean category would remove, or None if it removes no packages."""
    if cid == "autoremove":
        return apt_sim(["autoremove"])[1]
    if cid == "old-kernels":
        pkgs = old_kernel_pkgs(installed_pkgs(KERNEL_GLOBS), os.uname().release)
        return removal_plan(pkgs, True)[0] if pkgs else []
    if cid == "locales":
        pkgs = locale_pkgs_to_remove(installed_pkgs(LOCALE_GLOBS), used_langs())
        return removal_plan(pkgs, True)[0] if pkgs else []
    if cid == "rc-packages":
        return rc_packages()
    return None


def installed_snaps():
    if not have("snap"):
        return set()
    code, out = run(["snap", "list"], timeout=30)
    return {l.split()[0] for l in out.splitlines()[1:] if l.split()}


def installed_flatpaks():
    if not have("flatpak"):
        return set()
    code, out = run(["flatpak", "list", "--app", "--columns=application"], timeout=30)
    return {l.strip() for l in out.splitlines() if l.strip()}


def app_states():
    apt_ok = installed_apt([a.pid for a in APPS if a.kind == "apt"])
    snaps = installed_snaps()
    flats = installed_flatpaks()
    st = {}
    for a in APPS:
        st[a.key] = (a.pid in apt_ok if a.kind == "apt" else
                     a.pid in snaps if a.kind == "snap" else a.pid in flats)
    return st


def bloat_states():
    allpkgs = [p for b in BLOAT for p in b.apt]
    apt_ok = installed_apt(allpkgs)
    snaps = installed_snaps()
    return {b.id: (any(p in apt_ok for p in b.apt) or any(s in snaps for s in b.snap))
            for b in BLOAT}


def flatpak_argv(a, install):
    if install:
        return ["flatpak", "install", "--user", "-y", "--noninteractive", "flathub", a.pid]
    return ["flatpak", "uninstall", "-y", "--noninteractive", a.pid]


# --------------------------------------------------------------------------
# Root operations (validated in the helper)
# --------------------------------------------------------------------------
def valid_op(op):
    """True if op is a well-formed root operation the helper accepts."""
    kind, _, arg = op.partition(":")
    if kind in ("tweak-apply", "tweak-revert"):
        return arg in TWEAK_BY_ID and TWEAK_BY_ID[arg].root
    if kind == "clean":
        return arg in ROOT_CLEAN
    if kind == "util":
        return arg in UTIL_BY_ID and UTIL_BY_ID[arg].root
    if kind in ("apt-install", "apt-remove"):
        return arg in {a.pid for a in APPS if a.kind == "apt"} or \
            arg in {p for b in BLOAT for p in b.apt}
    if kind in ("snap-install", "snap-remove"):
        return arg in {a.pid for a in APPS if a.kind == "snap"} or \
            arg in {s for b in BLOAT for s in b.snap}
    if kind in ("bloat-remove", "bloat-purge", "bloat-restore"):
        return arg in BLOAT_BY_ID
    if kind in ("snapshot", "snap-purge", "snap-restore"):
        return arg == "" and op == kind
    return False


# --------------------------------------------------------------------------
# System info
# --------------------------------------------------------------------------
def read(path, default=""):
    try:
        with open(path) as f:
            return f.read()
    except OSError:
        return default


def os_name():
    m = re.search(r'^PRETTY_NAME="?([^"\n]*)', read("/etc/os-release"), re.M)
    return m.group(1) if m else "Linux"


def cpu_model():
    m = re.search(r"^model name\s*:\s*(.+)$", read("/proc/cpuinfo"), re.M)
    name = m.group(1) if m else "CPU"
    return re.sub(r"\s+", " ", re.sub(r"\((R|TM)\)|CPU|Processor|\d+-Core", "", name)).strip()


def cpu_times():
    parts = read("/proc/stat").split("\n", 1)[0].split()[1:]
    vals = [int(x) for x in parts]
    idle = vals[3] + (vals[4] if len(vals) > 4 else 0)
    return sum(vals), idle


def meminfo():
    info = {}
    for line in read("/proc/meminfo").splitlines():
        k, _, v = line.partition(":")
        if v.split():
            info[k] = int(v.split()[0]) * 1024
    total = info.get("MemTotal", 0)
    avail = info.get("MemAvailable", 0)
    return total, total - avail, info.get("SwapTotal", 0), info.get("SwapTotal", 0) - info.get("SwapFree", 0)


def gpus():
    if not have("lspci"):
        return []
    code, out = run(["lspci"], timeout=10)
    res = []
    for line in out.splitlines():
        if re.search(r"VGA|3D controller|Display controller", line):
            name = line.split(":", 2)[-1].strip()
            name = re.sub(r"\(rev .*\)", "", name).strip()
            m = re.search(r"\[([^\]]+)\]\s*$", name)
            vendor = name.split()[0]
            res.append(f"{vendor} {m.group(1)}" if m else name)
    return res


def uptime_text():
    try:
        secs = int(float(read("/proc/uptime").split()[0]))
    except (IndexError, ValueError):
        return "?"
    d, r = divmod(secs, 86400)
    h, r = divmod(r, 3600)
    m = r // 60
    return (f"{d} d " if d else "") + f"{h} h {m} min"


def human(n):
    if n is None:
        return "unknown"
    for unit in ("B", "KB", "MB", "GB", "TB"):
        if abs(n) < 1024 or unit == "TB":
            return f"{n:.0f} {unit}" if unit in ("B", "KB") else f"{n:.1f} {unit}"
        n /= 1024


# --------------------------------------------------------------------------
# Settings and history (user)
# --------------------------------------------------------------------------
def data_dir():
    d = os.path.join(os.environ.get("XDG_DATA_HOME") or os.path.join(HOME, ".local/share"), "glint")
    os.makedirs(d, exist_ok=True)
    return d


def load_state():
    try:
        with open(os.path.join(data_dir(), "state.json")) as f:
            st = json.load(f)
    except (OSError, ValueError):
        st = {}
    st.setdefault("applied", [])
    st.setdefault("history", [])
    st.setdefault("snapshot_first", True)
    return st


def save_state(st):
    st["history"] = st["history"][-300:]
    path = os.path.join(data_dir(), "state.json")
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        json.dump(st, f, indent=1)
    os.replace(tmp, path)


def record(st, tweak_id, applied):
    if applied and tweak_id not in st["applied"]:
        st["applied"].append(tweak_id)
    if not applied and tweak_id in st["applied"]:
        st["applied"].remove(tweak_id)
    st["history"].append({"t": int(time.time()), "id": tweak_id,
                          "action": "apply" if applied else "revert"})
