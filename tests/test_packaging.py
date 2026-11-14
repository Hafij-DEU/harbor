"""Every relative import (also the ones inside functions) must resolve to a
module that build-deb.sh puts into that app's package."""

import ast
import glob
import os

SRC = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "src", "harbor")

# keep in sync with build-deb.sh
EXTRA = {"files": ["qr", "taskmgr_ui"], "taskmgr": []}


def package_modules(app):
    mods = {"__init__", "common", app} | set(EXTRA[app])
    mods |= {os.path.basename(f)[:-3] for f in glob.glob(os.path.join(SRC, app + "_*.py"))}
    return mods


def relative_imports(mod):
    tree = ast.parse(open(os.path.join(SRC, mod + ".py")).read())
    for node in ast.walk(tree):
        if isinstance(node, ast.ImportFrom) and node.level == 1:
            if node.module:
                yield node.module.split(".")[0]
            else:
                yield from (a.name for a in node.names)


def test_packages_are_self_contained():
    for app in EXTRA:
        mods = package_modules(app)
        init = ast.parse(open(os.path.join(SRC, "__init__.py")).read())
        names = {t.id for n in init.body if isinstance(n, ast.Assign) for t in n.targets}
        missing = {(m, imp) for m in mods for imp in relative_imports(m)
                   if imp not in mods | names}
        assert not missing, (app, sorted(missing))
