"""Harbor Settings bridge to the Glint tweak/DNS catalog (glint_core.py,
tweak_helper.py copied from Glint). Harbor runs it as
  python3 -I -c BRIDGE CORE_SRC HELPER_SRC MODE [ARGS...]
and, for root tweaks, the same through pkexec. Every root operation is
checked with glint_core.valid_op, so only catalog operations ever run.
"""
import json
import os
import subprocess
import sys
import types
from concurrent.futures import ThreadPoolExecutor


def load(name, src):
    mod = types.ModuleType(name)
    mod.__file__ = name + ".py"
    sys.modules[name] = mod
    exec(compile(src, name + ".py", "exec"), mod.__dict__)
    return mod


core = load("glint_core", sys.argv[1])
helper_src = sys.argv[2]
mode = sys.argv[3]
args = sys.argv[4:]


# Risk of turning a tweak ON: "safe" (green), "warning" (orange) or
# "critical" (red), with what can go wrong. Unlisted tweaks are safe.
RISK = {
    "unattended": ("critical", "Security fixes stop installing by themselves; you must update by hand or stay exposed."),
    "firewall": ("warning", "Blocks incoming connections: MeshDrop, Wi-Fi file sharing and other LAN apps need their ports opened."),
    "grub-timeout": ("warning", "Only 1 s to pick another kernel or recovery mode if a boot goes wrong (hold Shift/Esc)."),
    "nmi-watchdog": ("warning", "Hard system freezes are no longer detected and logged."),
    "bluetooth": ("warning", "Bluetooth headphones, mice and keyboards stop working."),
    "apt-timers": ("warning", "Updates are no longer downloaded daily, so update notices come late."),
    "snap-hold": ("warning", "Snap apps get no updates, including security fixes, until you update them."),
    "pro-services": ("warning", "With Ubuntu Pro attached, ESM security updates and Livepatch notices stop."),
    "fwupd-timer": ("warning", "You are no longer told about new firmware (BIOS, SSD, dock) updates."),
    "connectivity": ("warning", "Hotel and café Wi-Fi sign-in pages no longer open by themselves."),
    "dot": ("warning", "Some networks block encrypted DNS; name lookups can get slower there."),
    "cups": ("warning", "You can no longer print or scan to network printers."),
    "avahi": ("warning", "Network printers, Chromecasts and .local names are no longer found; some LAN apps may not see this PC."),
    "cups-browsed": ("warning", "New network printers are not added automatically."),
    "indexer": ("warning", "Search in Files and Activities only finds file names, not file contents."),
    "wait-online": ("warning", "Network drives mounted at boot may fail if the network is not up yet."),
    "fast-shutdown": ("warning", "Slow services get only 10 s to save their data before they are stopped."),
    "geoclue": ("warning", "Automatic time zone, weather and maps location stop working."),
    "as-geoclue": ("warning", "Apps that ask for your location get no answer."),
    "as-update-notifier": ("warning", "You are no longer reminded to install updates."),
    "as-dejadup": ("warning", "Scheduled Déjà Dup backups no longer run by themselves."),
    "as-evolution": ("warning", "Calendar and task reminders no longer pop up."),
    "overamp": ("warning", "Sound above 100% can distort and strain small laptop speakers."),
    "recent-files": ("safe", "File pickers stop showing recent files; nothing is deleted."),
    "wifi-powersave": ("safe", "Wi-Fi uses a little more battery."),
    "modemmanager": ("safe", "Only matters if you use a USB or built-in mobile modem."),
    "as-orca": ("safe", "Only matters if you use the Orca screen reader."),
    "animations": ("safe", "Windows open and close without motion."),
}
SAFE_REASON = ""  # plain safe rows get only the green pill


def out(obj):
    print(json.dumps(obj), flush=True)


def tweak_list():
    def probe(t):
        try:
            if not core.tweak_available(t):
                return None
            return {"id": t.id, "cat": t.cat, "title": t.title, "desc": t.desc,
                    "warn": t.warn, "rec": t.rec, "root": t.root,
                    "applied": bool(core.tweak_applied(t)),
                    "risk": RISK.get(t.id, ("safe", SAFE_REASON))[0],
                    "why": RISK.get(t.id, ("safe", SAFE_REASON))[1]}
        except Exception:
            return None
    with ThreadPoolExecutor(8) as ex:
        rows = [r for r in ex.map(probe, core.TWEAKS) if r]
    out({"cats": [{"id": c, "title": t, "desc": d} for c, t, d in core.TWEAK_CATS],
         "tweaks": rows})


def user_tweak(tid, apply):
    t = core.TWEAK_BY_ID.get(tid)
    if not t or t.root:
        print(f"refused {tid}", flush=True)
        return False
    ok = True
    for argv in core.user_tweak_steps(t, apply):
        print("$ " + " ".join(argv), flush=True)
        r = subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        if r.stdout.strip():
            print(r.stdout.strip(), flush=True)
        ok &= r.returncode == 0
    return ok


def root_ops(ops):
    if os.geteuid() != 0:
        print("root tweaks need administrator rights", flush=True)
        return 2
    bad = [o for o in ops if not core.valid_op(o) or not o.startswith(("tweak-apply:", "tweak-revert:"))]
    if bad:
        print("refused: " + " ".join(bad), flush=True)
        return 3
    helper = load("tweak_helper", helper_src)
    failed = 0
    for op in ops:
        print(f"== {op}", flush=True)
        try:
            ok = helper.do_op(op)
        except Exception as e:
            print(f"error: {e}", flush=True)
            ok = False
        failed += not ok
    return 1 if failed else 0


def dns_info():
    conn = core.active_connection()
    out({"conn": list(conn) if conn else None,
         "current": core.current_dns(),
         "presets": [{"id": i, "title": t, "desc": d, "servers": s}
                     for i, t, d, s in core.DNS_PRESETS]})


def dns_test(servers):
    servers = [s for s in servers if core.IPV4_RE.match(s)]
    with ThreadPoolExecutor(8) as ex:
        res = dict(zip(servers, ex.map(core.dns_latency, servers)))
    out(res)


def dns_set(uuid, servers):
    if not all(core.IPV4_RE.match(s) for s in servers):
        print("refused: not an IPv4 address", flush=True)
        return 3
    conn = core.active_connection()
    if not conn or conn[0] != uuid:
        print("the active connection changed; reopen the page", flush=True)
        return 3
    for argv in core.dns_set_argv(uuid, servers):
        print("$ " + " ".join(argv), flush=True)
        r = subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        if r.returncode != 0:
            print(r.stdout.strip(), flush=True)
            return 1
    return 0


if mode == "tweaks":
    tweak_list()
elif mode == "user":
    ok = all([user_tweak(a.split(":", 1)[1], a.startswith("apply:")) for a in args])
    sys.exit(0 if ok else 1)
elif mode == "root":
    sys.exit(root_ops(args))
elif mode == "dns-info":
    dns_info()
elif mode == "dns-test":
    dns_test(args)
elif mode == "dns-set":
    sys.exit(dns_set(args[0], args[1:]))
elif mode == "dns-flush":
    r = subprocess.run(["resolvectl", "flush-caches"], stdout=subprocess.PIPE,
                       stderr=subprocess.STDOUT, text=True)
    print(r.stdout.strip() or "DNS cache cleared", flush=True)
    sys.exit(r.returncode)
else:
    sys.exit(2)
