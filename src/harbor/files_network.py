"""Harbor Files: saved network locations (FTP, SFTP, SMB, WebDAV, NFS, AFP).

Locations are opened through GIO/gvfs. Passwords are asked by
Gtk.MountOperation and, when the user ticks "Remember", kept by the GNOME
keyring. Harbor's settings file only ever holds the URL without a password.
"""

import re
from urllib.parse import quote, unquote, urlsplit

from gi.repository import Adw, Gtk

from . import files_core as fc

# (scheme, label, default port)
PROTOCOLS = [
    ("sftp", "SFTP (SSH)", 22),
    ("smb", "Windows share (SMB)", 445),
    ("ftp", "FTP", 21),
    ("ftps", "FTP over TLS (FTPS)", 990),
    ("davs", "WebDAV over HTTPS", 443),
    ("dav", "WebDAV", 80),
    ("nfs", "NFS", 2049),
    ("afp", "Apple share (AFP)", 548),
]
SCHEMES = {p[0] for p in PROTOCOLS}
ALIASES = {"ssh": "sftp", "webdav": "dav", "webdavs": "davs", "cifs": "smb",
           "https": "davs", "http": "dav"}


def normalize_url(text):
    """Clean what the user typed into a location URL, or raise ValueError.

    Accepts "host" (assumed sftp), "scheme://host/path", Windows style
    "\\\\host\\share", and the ssh/webdav/cifs aliases. Passwords are dropped.
    """
    t = (text or "").strip()
    if not t:
        raise ValueError("Enter a server address")
    if t.startswith("\\\\"):
        t = "smb://" + t[2:].replace("\\", "/")
    m = re.match(r"^([a-zA-Z][a-zA-Z0-9+.-]*)://(.*)$", t)
    if m:
        scheme, rest = m.group(1).lower(), m.group(2)
    else:
        scheme, rest = "sftp", t
    scheme = ALIASES.get(scheme, scheme)
    if scheme not in SCHEMES:
        raise ValueError("Harbor can't open %s:// addresses" % scheme)
    url = fc.strip_url_password(scheme + "://" + rest)
    host = urlsplit(url).hostname
    if not host or any(c.isspace() for c in rest.split("/", 1)[0]):
        raise ValueError("The address has no server name")
    if "/" not in rest:
        url += "/"
    return url


def build_url(scheme, server, port=None, folder="", user=""):
    """Assemble a URL from the dialog fields (no password, ever)."""
    server = (server or "").strip().strip("/")
    if not server:
        raise ValueError("Enter a server name or address")
    if ":" in server and not server.startswith("["):
        server = "[%s]" % server if server.count(":") > 1 else server
    defaults = {p[0]: p[2] for p in PROTOCOLS}
    netloc = server
    if port and int(port) != defaults.get(scheme):
        netloc += ":%d" % int(port)
    if user:
        netloc = quote(user.strip(), safe="") + "@" + netloc
    folder = (folder or "").strip().strip("/")
    path = "/" + quote(folder, safe="/") + ("/" if folder else "")
    return normalize_url("%s://%s%s" % (scheme, netloc, path))


def display_name(url, name=""):
    """Sidebar label: the saved name, else "host" or "share on host"."""
    if name:
        return name
    try:
        parts = urlsplit(url)
    except ValueError:
        return url
    host = parts.hostname or url
    last = unquote(parts.path.strip("/").split("/")[-1]) if parts.path.strip("/") else ""
    return "%s on %s" % (last, host) if last else host


def icon_for(url):
    return "folder-remote-symbolic"


class AddLocationDialog(Adw.AlertDialog):
    """Add (or edit) a network location. on_save(url, name)."""

    def __init__(self, on_save, url="", name=""):
        super().__init__(heading="Add network location",
                         body="Passwords are asked when you connect and kept by the "
                              "GNOME keyring if you choose, never by Harbor.")
        self.on_save = on_save
        self.add_response("cancel", "Cancel")
        self.add_response("save", "Add")
        self.set_response_appearance("save", Adw.ResponseAppearance.SUGGESTED)
        self.set_default_response("save")
        self.set_close_response("cancel")

        box = Gtk.ListBox(css_classes=["boxed-list"], selection_mode=Gtk.SelectionMode.NONE)
        self.proto = Adw.ComboRow(title="Type", model=Gtk.StringList.new(
            [p[1] for p in PROTOCOLS]))
        self.server = Adw.EntryRow(title="Server (name or IP)")
        self.port = Adw.SpinRow.new_with_range(1, 65535, 1)
        self.port.set_title("Port")
        self.port.set_value(PROTOCOLS[0][2])
        self.folder = Adw.EntryRow(title="Folder or share (optional)")
        self.user = Adw.EntryRow(title="User name (optional)")
        self.name = Adw.EntryRow(title="Name in the sidebar (optional)", text=name or "")
        self.addr = Adw.EntryRow(title="…or paste an address (ftp://, smb://, sftp://…)")
        for r in (self.proto, self.server, self.port, self.folder, self.user, self.name,
                  self.addr):
            box.append(r)
        self.proto.connect("notify::selected", self._proto_changed)
        self.err = Gtk.Label(css_classes=["error", "caption"], wrap=True, visible=False,
                             xalign=0, margin_top=6)
        outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        outer.append(box)
        outer.append(self.err)
        self.set_extra_child(outer)
        if url:
            self.set_heading("Edit network location")
            self.set_response_label("save", "Save")
            self.addr.set_text(url)
        for r in (self.server, self.folder, self.user, self.addr):
            r.connect("changed", lambda *_: self._validate())
        self.port.connect("notify::value", lambda *_: self._validate())
        self.connect("response", self._response)
        self._validate()

    def _proto_changed(self, row, _p):
        self.port.set_value(PROTOCOLS[row.get_selected()][2])
        self._validate()

    def _validate(self):
        try:
            url = self.url()
            msg = url
            ok = True
        except ValueError as e:
            msg, ok = str(e), False
        # nothing typed yet: no red text
        typed = self.server.get_text().strip() or self.addr.get_text().strip()
        self.err.set_label(msg)
        self.err.set_css_classes(["caption"] + ([] if ok else ["error"]))
        self.err.set_visible(bool(typed))
        self.set_response_enabled("save", ok)

    def url(self):
        if self.addr.get_text().strip():
            return normalize_url(self.addr.get_text())
        scheme = PROTOCOLS[self.proto.get_selected()][0]
        return build_url(scheme, self.server.get_text(), int(self.port.get_value()),
                         self.folder.get_text(), self.user.get_text())

    def _response(self, _d, resp):
        if resp != "save":
            return
        try:
            url = self.url()
        except ValueError:
            return
        self.on_save(url, self.name.get_text().strip())
