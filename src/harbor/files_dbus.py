"""Harbor Files: org.freedesktop.FileManager1 on the session bus.

Apps call ShowFolders / ShowItems / ShowItemProperties to "show in folder".
Harbor only takes the name while it is running and only when nobody else
owns it (DO_NOT_QUEUE, no replace); it never installs a D-Bus service file,
so Nautilus stays the activatable handler. If the name is taken, Harbor
logs it once and carries on.
"""

import sys

from gi.repository import Gio, GLib

from . import files_core as fc

BUS_NAME = "org.freedesktop.FileManager1"
OBJECT_PATH = "/org/freedesktop/FileManager1"
INTERFACE = "org.freedesktop.FileManager1"

XML = """
<node>
  <interface name="org.freedesktop.FileManager1">
    <method name="ShowFolders">
      <arg type="as" name="URIs" direction="in"/>
      <arg type="s" name="StartupId" direction="in"/>
    </method>
    <method name="ShowItems">
      <arg type="as" name="URIs" direction="in"/>
      <arg type="s" name="StartupId" direction="in"/>
    </method>
    <method name="ShowItemProperties">
      <arg type="as" name="URIs" direction="in"/>
      <arg type="s" name="StartupId" direction="in"/>
    </method>
  </interface>
</node>
"""


class FileManager1:
    """Owns the name for `app` while possible. show(method, targets) is
    called on the main loop with fc.fm1_targets() output."""

    def __init__(self, app, show):
        self.app = app
        self.show = show
        self.owner_id = 0
        self.reg_id = 0
        self.owned = False
        self.conn = None
        self._node = Gio.DBusNodeInfo.new_for_xml(XML)

    def start(self):
        self.owner_id = Gio.bus_own_name(
            Gio.BusType.SESSION, BUS_NAME, Gio.BusNameOwnerFlags.DO_NOT_QUEUE,
            self._on_bus, self._on_acquired, self._on_lost)
        return self.owner_id

    def stop(self):
        if self.reg_id and self.conn is not None:
            self.conn.unregister_object(self.reg_id)
            self.reg_id = 0
        if self.owner_id:
            Gio.bus_unown_name(self.owner_id)
            self.owner_id = 0
        self.owned = False

    def _on_bus(self, conn, _name):
        self.conn = conn
        try:
            self.reg_id = conn.register_object(OBJECT_PATH, self._node.interfaces[0],
                                               self._on_call, None, None)
        except GLib.Error as e:
            print("harbor-files: can't export %s: %s" % (OBJECT_PATH, e.message), file=sys.stderr)

    def _on_acquired(self, _conn, _name):
        self.owned = True

    def _on_lost(self, conn, _name):
        # Someone else (usually Nautilus) has it, or there is no bus. Fine.
        if not self.owned:
            print("harbor-files: %s is owned by another app; not exporting it" % BUS_NAME,
                  file=sys.stderr)
        self.owned = False
        if self.reg_id and conn is not None:
            conn.unregister_object(self.reg_id)
            self.reg_id = 0

    def _on_call(self, _conn, _sender, _path, _iface, method, params, invocation):
        try:
            uris, _startup = params.unpack()
            targets = fc.fm1_targets(method, list(uris))
        except (ValueError, TypeError) as e:
            invocation.return_dbus_error("org.freedesktop.DBus.Error.InvalidArgs", str(e))
            return
        invocation.return_value(None)
        GLib.idle_add(lambda: (self.show(method, targets), False)[1])
