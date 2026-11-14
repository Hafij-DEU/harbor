"""Harbor Files: 7-Zip integration (no widgets).

Builds 7z argument lists, parses its output and runs it in a worker
thread. Rules:

* argv lists only, never a shell; every path goes after "--";
* a password is passed as one "-p<pw>" argument, never logged (redact()),
  and "-p" (empty) is always passed so 7z never waits for a typed password;
* existing files are never overwritten silently: -aos (skip) unless the
  user picked Overwrite (-aoa) or Rename (-aou);
* every member name is checked for zip-slip before anything is extracted;
* HARBOR_DRY_RUN=1 writes nothing: the redacted command is logged instead.
"""

import os
import re
import select
import shutil
import subprocess

from . import common
from . import files_core as fc

EXECUTABLES = ("7z", "7zz", "7za")

# type -> file name endings (lower case); longest match wins
TYPES = {
    "tar.gz": (".tar.gz", ".tgz"), "tar.xz": (".tar.xz", ".txz"),
    "tar.bz2": (".tar.bz2", ".tbz2", ".tbz"), "tar.zst": (".tar.zst", ".tzst"),
    "zip": (".zip",), "7z": (".7z",), "rar": (".rar",), "tar": (".tar",),
    "gz": (".gz",), "bz2": (".bz2",), "xz": (".xz",), "zst": (".zst",),
    "iso": (".iso",), "cab": (".cab",), "arj": (".arj",), "lzh": (".lzh", ".lha"),
    "wim": (".wim",), "deb": (".deb",), "rpm": (".rpm",), "apk": (".apk",),
    "jar": (".jar",),
}
_ENDINGS = sorted(((e, t) for t, es in TYPES.items() for e in es), key=lambda x: -len(x[0]))
# multi-volume first parts (".7z.001", ".part1.rar", ".zip.001")
_VOLUME = re.compile(r"\.(7z|zip|rar)\.0*1$|\.part0*1\.rar$", re.I)

FORMATS = ("7z", "zip", "tar", "tar.gz", "tar.xz", "tar.zst")
FORMAT_EXT = {"7z": ".7z", "zip": ".zip", "tar": ".tar", "tar.gz": ".tar.gz",
              "tar.xz": ".tar.xz", "tar.zst": ".tar.zst"}
LEVELS = (("Store", 0), ("Fastest", 1), ("Fast", 3), ("Normal", 5), ("Maximum", 7),
          ("Ultra", 9))
VOLUME_SIZES = (("No split", ""), ("10 MB", "10m"), ("100 MB", "100m"), ("700 MB (CD)", "700m"),
                ("1 GB", "1g"), ("4 GB (FAT32)", "4092m"))
COLLISIONS = {"overwrite": "-aoa", "skip": "-aos", "rename": "-aou"}


def archive_type(name):
    """Archive type from the file name ("zip", "7z", "tar.gz", ...) or None."""
    low = (name or "").lower()
    if _VOLUME.search(low):
        return low.rsplit(".", 2)[-2] if not low.endswith(".rar") else "rar"
    for ending, t in _ENDINGS:
        if low.endswith(ending) and len(low) > len(ending):
            return t
    return None


def archive_stem(name):
    low = name.lower()
    m = _VOLUME.search(low)
    if m:
        return name[:m.start()]
    for ending, _t in _ENDINGS:
        if low.endswith(ending) and len(name) > len(ending):
            return name[:-len(ending)]
    return fc.split_ext(name)[0]


def find_7z(which=shutil.which):
    for exe in EXECUTABLES:
        p = which(exe)
        if p:
            return p
    return None


def _arg_path(p):
    """Paths go after "--" anyway; a leading "-" is made absolute too so a
    7z without "--" support could never read it as a switch."""
    return os.path.abspath(p) if p.startswith("-") else p


def _pw(password):
    return "-p" + (password or "")


def redact(argv):
    """Printable command line with any password replaced."""
    out = []
    after = False
    for a in argv:
        if a == "--":
            after = True
        if not after and a.startswith("-p") and len(a) > 2:
            a = "-p***"
        out.append(a)
    import shlex
    return " ".join(shlex.quote(a) for a in out)


# ------------------------------------------------------------------ argv builders

def list_argv(exe, archive, password=None):
    return [exe, "l", "-slt", "-ba", "-sccUTF-8", _pw(password), "--", _arg_path(archive)]


def test_argv(exe, archive, password=None):
    return [exe, "t", "-bsp1", "-bso0", "-sccUTF-8", _pw(password), "--", _arg_path(archive)]


def extract_argv(exe, archive, dest, password=None, collision="skip", members=()):
    if collision not in COLLISIONS:
        raise ValueError("collision must be overwrite, skip or rename")
    return ([exe, "x", "-bsp1", "-bso0", "-sccUTF-8", COLLISIONS[collision],
             "-o" + os.path.abspath(dest), _pw(password), "--", _arg_path(archive)]
            + [m for m in members])


def add_argv(exe, dest_archive, names, fmt="7z", level=5, password=None, encrypt_names=False,
             volume="", delete_after=False):
    """7z / zip / tar: names are relative to the working directory the
    command runs in (the folder of the first item), so the archive holds
    plain names, not absolute paths."""
    if fmt not in ("7z", "zip", "tar"):
        raise ValueError("7z writes 7z, zip and tar; use tar_compress for %s" % fmt)
    if not names:
        raise ValueError("nothing to add")
    argv = [exe, "a", "-t" + fmt, "-bsp1", "-bso0", "-sccUTF-8"]
    if fmt != "tar":
        argv.append("-mx=%d" % int(level))
    if password and fmt != "tar":
        argv.append(_pw(password))
        if fmt == "7z" and encrypt_names:
            argv.append("-mhe=on")
        if fmt == "zip":
            argv.append("-mem=AES256")
    if volume:
        if not re.match(r"^\d+[bkmg]?$", volume):
            raise ValueError("bad volume size")
        argv.append("-v" + volume)
    if delete_after:
        argv.append("-sdel")
    argv += ["--", _arg_path(dest_archive)] + [_arg_path(n) for n in names]
    return argv


def hash_argv(exe, paths):
    return [exe, "h", "-scrcCRC32", "-scrcSHA256", "-sccUTF-8", "--"] + \
        [_arg_path(p) for p in paths]


# ------------------------------------------------------------------ output parsing

_PCT = re.compile(r"(\d{1,3})%")


def parse_progress(chunk):
    """Last percentage in a chunk of -bsp1 output (lines end in \\r or \\b), or None."""
    found = _PCT.findall(chunk or "")
    if not found:
        return None
    v = int(found[-1])
    return v if 0 <= v <= 100 else None


def parse_slt(text):
    """`7z l -slt -ba` output -> [{"path", "size", "is_dir", "modified", "encrypted"}]."""
    out = []
    cur = {}

    def flush():
        if "Path" in cur:
            attrs = cur.get("Attributes", "")
            out.append({"path": cur["Path"],
                        "size": int(cur.get("Size") or 0) if (cur.get("Size") or "0").isdigit()
                        else 0,
                        "is_dir": cur.get("Folder") == "+" or attrs.startswith("D"),
                        "modified": cur.get("Modified", ""),
                        "encrypted": cur.get("Encrypted") == "+"})
        cur.clear()
    for line in (text or "").splitlines():
        if not line.strip():
            flush()
            continue
        k, sep, v = line.partition(" = ")
        if not sep:
            continue
        if k == "Path" and "Path" in cur:
            flush()
        cur[k] = v
    flush()
    return out


def parse_hashes(text):
    """`7z h -scrcCRC32 -scrcSHA256` -> [(name, crc32, sha256)] per file."""
    rows = []
    in_table = False
    for line in (text or "").splitlines():
        if line.startswith("--------"):
            in_table = not in_table
            continue
        if in_table:
            m = re.match(r"^([0-9A-Fa-f]{8}) ([0-9A-Fa-f]{64}) +(\d*) {2}(.*)$", line)
            if m:
                rows.append((m.group(4), m.group(1).upper(), m.group(2).lower()))
    return rows


def wrong_password(text):
    t = (text or "").lower()
    return "wrong password" in t or "enter password" in t or "can not open encrypted" in t \
        or "cannot open encrypted" in t


def check_members(dest, members):
    """Refuse the whole archive when any member would land outside dest."""
    for m in members:
        fc.safe_member_path(dest, m["path"] if isinstance(m, dict) else m)


def collisions(dest, members):
    """Top-level names in members that already exist in dest."""
    tops = set()
    for m in members:
        p = (m["path"] if isinstance(m, dict) else m).replace("\\", "/").strip("/")
        if p:
            tops.add(p.split("/", 1)[0])
    return sorted(t for t in tops if os.path.lexists(os.path.join(dest, t)))


# ------------------------------------------------------------------ running

class PasswordNeeded(Exception):
    pass


def run(argv, job=None, cwd=None, mutating=True, on_percent=None, timeout=None):
    """Run 7z; returns (returncode, stdout, stderr).

    Progress (-bsp1, on stdout) goes to job.done/job.total (0..100) and
    on_percent(pct). job.cancel() kills the process. With HARBOR_DRY_RUN=1
    a mutating command is only logged (redacted).
    """
    if common.DRY_RUN and mutating:
        common.DRY_RUN_LOG.append(["7z-dry-run", redact(argv)])
        import sys
        print("[dry-run]", redact(argv), file=sys.stderr)
        return 0, "", ""
    env = dict(os.environ, LC_ALL="C.UTF-8")
    p = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                         stderr=subprocess.PIPE, cwd=cwd, env=env)
    out, err = [], []
    fds = {p.stdout.fileno(): out, p.stderr.fileno(): err}
    if job is not None:
        job.total = 100
    import time
    start = time.monotonic()
    try:
        while fds:
            if job is not None and job.cancelled:
                p.kill()
                p.wait()
                raise fc.Cancelled()
            if timeout and time.monotonic() - start > timeout:
                p.kill()
                p.wait()
                raise TimeoutError("7-Zip took too long")
            ready, _w, _x = select.select(list(fds), [], [], 0.2)
            for fd in ready:
                data = os.read(fd, 65536)
                if not data:
                    del fds[fd]
                    continue
                text = data.decode("utf-8", "replace")
                fds[fd].append(text)
                if fd == p.stdout.fileno():
                    pct = parse_progress(text)
                    if pct is not None:
                        if job is not None:
                            job.done = pct
                            job.progress("")
                        if on_percent:
                            on_percent(pct)
        rc = p.wait()
    finally:
        p.stdout.close()
        p.stderr.close()
    return rc, "".join(out), "".join(err)


def list_members(exe, archive, password=None):
    rc, out, err = run(list_argv(exe, archive, password), mutating=False, timeout=60)
    if rc != 0:
        if wrong_password(out + err):
            raise PasswordNeeded()
        raise RuntimeError(_error_text(out + err) or "7-Zip can't read this archive")
    members = parse_slt(out)
    if any(m["encrypted"] for m in members) and password is None:
        raise PasswordNeeded()
    return members


def _error_text(text):
    for line in (text or "").splitlines():
        line = line.strip()
        if line.startswith("ERROR:") or line.startswith("Error:"):
            return line.split(":", 1)[1].strip()
    return ""


def extract(job, exe, archive, dest, password=None, collision="skip", members=None):
    """Worker: check members, then extract into dest (made if missing)."""
    if members is None:
        members = list_members(exe, archive, password)
    check_members(dest, members)
    if not common.DRY_RUN:
        os.makedirs(dest, exist_ok=True)
    rc, out, err = run(extract_argv(exe, archive, dest, password, collision), job)
    if rc != 0:
        if wrong_password(out + err):
            raise PasswordNeeded()
        raise RuntimeError(_error_text(out + err) or "7-Zip couldn't extract (code %d)" % rc)
    job.copied.append(dest)
    return dest


def test(job, exe, archive, password=None):
    rc, out, err = run(test_argv(exe, archive, password), job, mutating=False)
    if rc != 0 and wrong_password(out + err):
        raise PasswordNeeded()
    return rc == 0, _error_text(out + err)


def add(job, exe, paths, dest_archive, fmt="7z", level=5, password=None, encrypt_names=False,
        volume="", delete_after=False):
    """Worker: compress paths (all in one folder) into dest_archive."""
    if os.path.lexists(dest_archive) and not volume:
        raise FileExistsError("%s already exists" % os.path.basename(dest_archive))
    cwd = os.path.dirname(os.path.normpath(paths[0]))
    names = [os.path.relpath(os.path.normpath(p), cwd) for p in paths]
    if fmt in ("tar.gz", "tar.xz", "tar.zst"):
        return tar_compress(job, paths, dest_archive, fmt, delete_after)
    argv = add_argv(exe, dest_archive, names, fmt, level, password, encrypt_names, volume,
                    delete_after)
    try:
        rc, out, err = run(argv, job, cwd=cwd)
    except BaseException:
        if not common.DRY_RUN and os.path.exists(dest_archive) and not volume:
            os.unlink(dest_archive)
        raise
    if rc != 0:
        raise RuntimeError(_error_text(out + err) or "7-Zip couldn't compress (code %d)" % rc)
    job.copied.append(dest_archive if not volume else dest_archive + ".001")
    return dest_archive


def tar_compress(job, paths, dest, fmt, delete_after=False):
    """tar.gz / tar.xz / tar.zst with Python's tarfile (7-Zip can't write zstd)."""
    import tarfile
    mode = {"tar.gz": "w:gz", "tar.xz": "w:xz", "tar.zst": "w:zst"}[fmt]
    if common.DRY_RUN:
        common.DRY_RUN_LOG.append(["tar-dry-run", mode, dest] + list(paths))
        return dest
    total = 0
    for p in paths:
        total += fc.tree_size(p)[0] if os.path.isdir(p) else os.lstat(p).st_size
    job.total = max(total, 1)
    fd = os.open(dest, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    try:
        with os.fdopen(fd, "wb") as raw, tarfile.open(fileobj=raw, mode=mode) as tf:
            def add_one(full, arc):
                job.check()
                tf.add(full, arc, recursive=False)
                if os.path.isfile(full) and not os.path.islink(full):
                    job.done += os.lstat(full).st_size
                    job.progress(arc)
            for p in paths:
                p = os.path.normpath(p)
                base = os.path.dirname(p)
                add_one(p, os.path.basename(p))
                if os.path.isdir(p) and not os.path.islink(p):
                    for d, dirs, files in os.walk(p):
                        dirs.sort()
                        for n in dirs + sorted(files):
                            full = os.path.join(d, n)
                            add_one(full, os.path.relpath(full, base))
    except BaseException:
        try:
            os.unlink(dest)
        except OSError:
            pass
        raise
    if delete_after:
        for p in paths:
            if os.path.isdir(p) and not os.path.islink(p):
                shutil.rmtree(p)
            else:
                os.unlink(p)
    job.copied.append(dest)
    return dest


def hashes(exe, paths):
    rc, out, err = run(hash_argv(exe, paths), mutating=False, timeout=600)
    if rc != 0:
        raise RuntimeError(_error_text(out + err) or "7-Zip couldn't read the file")
    return parse_hashes(out)
