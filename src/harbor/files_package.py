"""Harbor Files: what a .deb or .apk is, for the preview pane.

deb_info() reads the control fields with dpkg-deb and asks dpkg which
version is installed; deb_icon() pulls the app icon out of the data part.
apk_info() reads AndroidManifest.xml (binary XML) and the app name from
resources.arsc with no Android tools. Everything here is blocking: run it
off the main loop.
"""

import os
import shutil
import struct
import subprocess
import tarfile
import zipfile

DEB_FIELDS = ("Package", "Version", "Architecture", "Installed-Size", "Depends",
              "Maintainer", "Homepage", "Description")
TIMEOUT = 10
ICON_TIMEOUT = 8
ICON_MAX_BYTES = 4 * 1024 * 1024


# ------------------------------------------------------------------ .deb
def parse_control(text):
    """{field: value} from deb control text; continuation lines join with
    newlines and a lone "." becomes an empty line."""
    out, key = {}, None
    for line in text.splitlines():
        if line[:1] in (" ", "\t") and key:
            body = line[1:]
            out[key] += "\n" + ("" if body.strip() == "." else body)
        elif ":" in line:
            key, _, val = line.partition(":")
            key = key.strip()
            out[key] = val.strip()
    return out


def _run(argv, timeout=TIMEOUT):
    try:
        p = subprocess.run(argv, capture_output=True, text=True, timeout=timeout)
    except (OSError, subprocess.TimeoutExpired):
        return None
    return p.stdout if p.returncode == 0 else None


def installed_version(package):
    """Version of an installed package, or None."""
    if not package or not shutil.which("dpkg-query"):
        return None
    out = _run(["dpkg-query", "-W", "-f=${db:Status-Status}\t${Version}", package])
    if not out:
        return None
    status, _, ver = out.partition("\t")
    return ver.strip() if status.strip() == "installed" else None


def compare_versions(a, b):
    """-1, 0 or 1 like dpkg --compare-versions (None when dpkg is missing)."""
    if not shutil.which("dpkg"):
        return None
    for res, op in ((0, "eq"), (1, "gt")):
        try:
            if subprocess.run(["dpkg", "--compare-versions", a, op, b],
                              timeout=TIMEOUT).returncode == 0:
                return res
        except (OSError, subprocess.TimeoutExpired):
            return None
    return -1


def install_state(file_version, installed):
    """not-installed / update / installed / newer-installed / unknown."""
    if not installed:
        return "not-installed"
    cmp = compare_versions(file_version, installed)
    if cmp is None:
        return "unknown"
    return {1: "update", 0: "installed", -1: "newer-installed"}[cmp]


_ARCH = []


def native_arch():
    if not _ARCH:
        _ARCH.append((_run(["dpkg", "--print-architecture"]) or "").strip())
    return _ARCH[0]


def deb_info(path):
    """Control fields plus "installed" (version or None) and "state"; None if
    this is not a readable .deb."""
    out = _run(["dpkg-deb", "-f", path] + list(DEB_FIELDS))
    if not out:
        return None
    info = parse_control(out)
    if not info.get("Package"):
        return None
    inst = installed_version(info["Package"])
    info["installed"] = inst
    info["state"] = install_state(info.get("Version", ""), inst)
    arch, native = info.get("Architecture", ""), native_arch()
    if arch and native and arch not in ("all", native):
        info["state"] = "wrong-arch"
    return info


def _icon_rank(name):
    """Higher is better; None for files that are not an app icon."""
    n = name.lstrip("./")
    low = n.lower()
    if not low.endswith((".png", ".svg")) or "symbolic" in low:
        return None
    if low.startswith("usr/share/icons/hicolor/") and "/apps/" in low:
        size = low.split("/")[4]
        if size == "scalable":
            return 900
        try:
            px = int(size.split("x")[0])
        except ValueError:
            return None
        return px if px <= 512 else 600 - px // 100
    if low.startswith("usr/share/pixmaps/"):
        return 100
    if low.startswith("opt/") and "/" not in low[4:].split("/", 1)[-1] and "icon" in low:
        return 50
    return None


def deb_icon(path):
    """(bytes, ".png"/".svg") of the biggest app icon in the package, or None.
    Streams the data tarball once; stops after ICON_TIMEOUT."""
    try:
        proc = subprocess.Popen(["dpkg-deb", "--fsys-tarfile", path], stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL)
    except OSError:
        return None
    best = None
    import time
    end = time.monotonic() + ICON_TIMEOUT
    try:
        with tarfile.open(fileobj=proc.stdout, mode="r|") as tar:
            for m in tar:
                if time.monotonic() > end:
                    break
                rank = _icon_rank(m.name)
                if rank is None or not m.isfile() or m.size > ICON_MAX_BYTES:
                    continue
                if best is None or rank > best[0]:
                    f = tar.extractfile(m)
                    if f is not None:
                        best = (rank, f.read(), os.path.splitext(m.name)[1].lower())
    except (tarfile.TarError, OSError, EOFError):
        pass
    finally:
        proc.kill()
        proc.wait()
    return (best[1], best[2]) if best else None


# ------------------------------------------------------------------ .apk
RES_STRING_POOL, RES_XML, RES_XML_START, RES_XML_MAP = 0x0001, 0x0003, 0x0102, 0x0180
RES_TABLE, RES_TABLE_PACKAGE, RES_TABLE_TYPE = 0x0002, 0x0200, 0x0201
ATTR_IDS = {0x0101021b: "versionCode", 0x0101021c: "versionName",
            0x0101020c: "minSdkVersion", 0x01010270: "targetSdkVersion",
            0x01010001: "label", 0x01010003: "name", 0x01010002: "icon"}
TYPE_REF, TYPE_STRING, TYPE_INT_DEC, TYPE_INT_HEX = 0x01, 0x03, 0x10, 0x11
ANDROID_NAMES = {21: "5.0", 22: "5.1", 23: "6", 24: "7.0", 25: "7.1", 26: "8.0", 27: "8.1",
                 28: "9", 29: "10", 30: "11", 31: "12", 32: "12L", 33: "13", 34: "14",
                 35: "15", 36: "16"}


def _string_pool(data, off):
    """List of strings in the pool chunk at off."""
    _t, hsize, _size, count, _styles, flags, str_start, _ = struct.unpack_from(
        "<HHIIIIII", data, off)
    utf8 = bool(flags & 0x100)
    offs = struct.unpack_from("<%dI" % count, data, off + hsize)
    base = off + str_start
    out = []
    for o in offs:
        p = base + o
        try:
            if utf8:
                n = data[p]
                p += 2 if n & 0x80 else 1          # utf-16 length, skipped
                n = data[p]
                if n & 0x80:
                    n = ((n & 0x7f) << 8) | data[p + 1]
                    p += 1
                p += 1
                out.append(data[p:p + n].decode("utf-8", "replace"))
            else:
                n = struct.unpack_from("<H", data, p)[0]
                if n & 0x8000:
                    n = ((n & 0x7fff) << 16) | struct.unpack_from("<H", data, p + 2)[0]
                    p += 2
                p += 2
                out.append(data[p:p + 2 * n].decode("utf-16-le", "replace"))
        except (IndexError, struct.error):
            out.append("")
    return out


def parse_axml(data):
    """[(tag, {attr: (type, value)})] for the start tags of a binary XML file.
    String values come back as str, ints as int, references as ("ref", id)."""
    if len(data) < 8 or struct.unpack_from("<H", data, 0)[0] != RES_XML:
        raise ValueError("not binary XML")
    strings, resmap, out = [], [], []
    off = struct.unpack_from("<H", data, 2)[0]
    while off + 8 <= len(data):
        ctype, hsize, size = struct.unpack_from("<HHI", data, off)
        if size < 8:
            break
        if ctype == RES_STRING_POOL:
            strings = _string_pool(data, off)
        elif ctype == RES_XML_MAP:
            n = (size - hsize) // 4
            resmap = list(struct.unpack_from("<%dI" % n, data, off + hsize))
        elif ctype == RES_XML_START:
            ext = off + hsize
            _ns, name, astart, asize, acount = struct.unpack_from("<iiHHH", data, ext)
            tag = strings[name] if 0 <= name < len(strings) else ""
            attrs = {}
            for i in range(acount):
                a = ext + astart + i * asize
                _ans, aname, raw, _vs, _r, vtype, vdata = struct.unpack_from(
                    "<iiiHBBI", data, a)
                key = ATTR_IDS.get(resmap[aname]) if 0 <= aname < len(resmap) else None
                if key is None:
                    key = strings[aname] if 0 <= aname < len(strings) else ""
                if vtype == TYPE_STRING or (raw >= 0 and vtype != TYPE_REF):
                    val = strings[raw] if 0 <= raw < len(strings) else ""
                    if vtype in (TYPE_INT_DEC, TYPE_INT_HEX):
                        val = vdata
                elif vtype == TYPE_REF:
                    val = ("ref", vdata)
                elif vtype in (TYPE_INT_DEC, TYPE_INT_HEX):
                    val = vdata
                else:
                    val = vdata
                attrs[key] = val
            out.append((tag, attrs))
        off += size
    return out


def arsc_string(data, res_id):
    """The default-config string of resource res_id in resources.arsc, or None."""
    if len(data) < 12 or struct.unpack_from("<H", data, 0)[0] != RES_TABLE:
        return None
    pkg_id, type_id, entry = res_id >> 24, (res_id >> 16) & 0xff, res_id & 0xffff
    off = struct.unpack_from("<H", data, 2)[0]
    gstrings = []
    while off + 8 <= len(data):
        ctype, hsize, size = struct.unpack_from("<HHI", data, off)
        if size < 8:
            break
        if ctype == RES_STRING_POOL:
            gstrings = _string_pool(data, off)
        elif ctype == RES_TABLE_PACKAGE:
            if struct.unpack_from("<I", data, off + 8)[0] == pkg_id:
                found = _arsc_package_string(data, off, hsize, size, type_id, entry, gstrings)
                if found is not None:
                    return found
        off += size
    return None


def _arsc_package_string(data, pkg_off, hsize, size, type_id, entry, gstrings):
    off, end, best = pkg_off + hsize, pkg_off + size, None
    while off + 8 <= end:
        ctype, chsize, csize = struct.unpack_from("<HHI", data, off)
        if csize < 8:
            break
        if ctype == RES_TABLE_TYPE and data[off + 8] == type_id:
            flags = data[off + 9]
            count, start = struct.unpack_from("<II", data, off + 12)
            cfg = data[off + 20:off + 20 + struct.unpack_from("<I", data, off + 20)[0]]
            default = not any(cfg[4:])           # all-zero config = default
            eoff = None
            if flags & 0x01:                      # sparse
                for i in range(count):
                    idx, o16 = struct.unpack_from("<HH", data, off + chsize + 4 * i)
                    if idx == entry:
                        eoff = o16 * 4
                        break
            elif entry < count:
                o = struct.unpack_from("<I", data, off + chsize + 4 * entry)[0]
                if (flags & 0x02 and o != 0xffff) or (not flags & 0x02 and o != 0xffffffff):
                    eoff = o * 4 if flags & 0x02 else o
            if eoff is not None:
                e = off + start + eoff
                esize, eflags = struct.unpack_from("<HH", data, e)
                if not eflags & 0x0001:           # not a complex (bag) entry
                    vtype, vdata = data[e + esize + 3], struct.unpack_from(
                        "<I", data, e + esize + 4)[0]
                    if vtype == TYPE_STRING and vdata < len(gstrings):
                        if default:
                            return gstrings[vdata]
                        best = best or gstrings[vdata]
        off += csize
    return best


def apk_info(path):
    """{"package", "versionName", "versionCode", "minSdk", "targetSdk", "label",
    "permissions": [..]} for an APK, or None."""
    try:
        with zipfile.ZipFile(path) as z:
            tags = parse_axml(z.read("AndroidManifest.xml"))
            arsc = None
            names = set(z.namelist())
            info = {"permissions": [], "splits": sum(1 for n in names if n.endswith(".apk"))}
            for tag, a in tags:
                if tag == "manifest":
                    info["package"] = a.get("package", "")
                    info["versionName"] = a.get("versionName")
                    info["versionCode"] = a.get("versionCode")
                elif tag == "uses-sdk":
                    info["minSdk"] = a.get("minSdkVersion")
                    info["targetSdk"] = a.get("targetSdkVersion")
                elif tag == "uses-permission":
                    if isinstance(a.get("name"), str):
                        info["permissions"].append(a["name"])
                elif tag == "application":
                    label = a.get("label")
                    if isinstance(label, tuple):
                        if arsc is None and "resources.arsc" in names:
                            arsc = z.read("resources.arsc")
                        label = arsc_string(arsc, label[1]) if arsc else None
                    info["label"] = label if isinstance(label, str) else None
    except (OSError, KeyError, ValueError, zipfile.BadZipFile, struct.error, IndexError):
        return None
    if isinstance(info.get("versionName"), tuple):
        info["versionName"] = None
    return info if info.get("package") else None


def android_name(sdk):
    """'Android 8.0 (API 26)' for an int API level."""
    if not isinstance(sdk, int):
        return None
    name = ANDROID_NAMES.get(sdk)
    return "Android %s (API %d)" % (name, sdk) if name else "API %d" % sdk
