"""Harbor Files: menu models for the command bar and the context menus."""

from gi.repository import Gio, GLib

from . import files_core as fc

SORT_LABELS = (("name", "Name"), ("modified", "Date modified"), ("type", "Type"), ("size", "Size"))
VIEW_LABELS = (("large", "Large icons"), ("medium", "Medium icons"), ("details", "Details"))


def item(label, action, target=None, icon=None):
    mi = Gio.MenuItem.new(label, None)
    if target is None:
        mi.set_action_and_target_value(action, None)
    elif isinstance(target, bool):
        mi.set_action_and_target_value(action, GLib.Variant("b", target))
    else:
        mi.set_action_and_target_value(action, GLib.Variant("s", target))
    if icon:
        mi.set_icon(icon if isinstance(icon, Gio.Icon) else Gio.ThemedIcon.new(icon))
    return mi


def section(*items):
    m = Gio.Menu()
    for it in items:
        if it is not None:
            m.append_item(it)
    return m


def menu(*sections):
    m = Gio.Menu()
    for s in sections:
        if s is not None and s.get_n_items():
            m.append_section(None, s)
    return m


def new_menu():
    return menu(section(item("Folder", "win.new-folder", icon="folder-new-symbolic")),
                section(item("Text document", "win.new-text", icon="text-x-generic-symbolic"),
                        item("Empty file", "win.new-empty", icon="document-new-symbolic")))


def sort_menu():
    return menu(section(*[item(lab, "win.sort", key) for key, lab in SORT_LABELS]),
                section(item("Ascending", "win.sort-order", "asc"),
                        item("Descending", "win.sort-order", "desc")))


def view_menu():
    return menu(section(*[item(lab, "win.view", key) for key, lab in VIEW_LABELS]),
                section(item("Details pane", "win.details-pane"),
                        item("Hidden items", "win.show-hidden"),
                        item("File name extensions", "win.show-ext")))


def more_menu(pin_label="Pin to Quick access"):
    return menu(section(item("Select all", "win.select-all"),
                        item("Select none", "win.select-none"),
                        item("Invert selection", "win.invert-selection")),
                section(item(pin_label, "win.pin-toggle"),
                        item("Open in Terminal", "win.terminal"),
                        item("Copy path", "win.copy-path")),
                section(item("Undo", "win.undo")),
                section(item("Properties", "win.properties"),
                        item("About Harbor Files", "win.about")))


def open_with_menu(content_type, limit=12):
    """Submenu: apps for this type (default first) + "Choose another app"."""
    m = Gio.Menu()
    apps = []
    default = Gio.AppInfo.get_default_for_type(content_type, False) if content_type else None
    if default is not None:
        apps.append(default)
    seen = {a.get_id() for a in apps}
    for a in (Gio.AppInfo.get_all_for_type(content_type) if content_type else []):
        if a.get_id() not in seen and a.should_show():
            apps.append(a)
            seen.add(a.get_id())
    apps_sec = Gio.Menu()
    for a in apps[:limit]:
        if a.get_id():
            apps_sec.append_item(item(a.get_display_name() or a.get_name(), "win.open-with-app",
                                      a.get_id(), icon=a.get_icon()))
    m.append_section(None, apps_sec)
    m.append_section(None, section(item("Choose another app", "win.open-with-other")))
    return m


def item_menu(kind, sel, can_paste=False, pinned=False):
    """Context menu for selected items. kind as in fc.command_states."""
    st = fc.command_states(kind, sel, can_paste=can_paste)
    n = len(sel)
    one = sel[0] if n == 1 else {}
    if kind == "trash":
        return menu(section(item("Restore", "win.restore", icon="edit-undo-symbolic")),
                    section(item("Delete permanently", "win.delete-permanent")),
                    section(item("Properties", "win.properties")))
    s1 = section(
        item("Open", "win.open"),
        item("Open in new tab", "win.open-tab") if st["open-new-tab"] else None,
        item("Open file location", "win.open-location-of") if kind == "search" else None,
    )
    if st["open-with"]:
        s1.append_submenu("Open with", open_with_menu(one.get("content_type", "")))
    s2 = section(
        item("Cut", "win.cut", icon="edit-cut-symbolic") if st["cut"] else None,
        item("Copy", "win.copy", icon="edit-copy-symbolic") if st["copy"] else None,
        item("Paste into folder", "win.paste-into", icon="edit-paste-symbolic")
        if n == 1 and one.get("is_dir") and one.get("local") and can_paste else None,
    )
    s3 = section(
        item("Rename", "win.rename") if st["rename"] else None,
        item("Delete", "win.delete") if st["delete"] else None,
        item("Delete permanently", "win.delete-permanent") if st["delete-permanent"] else None,
    )
    s4 = section(
        item("Copy as path", "win.copy-path") if st["copy-path"] else None,
        item("Unpin from Quick access" if pinned else "Pin to Quick access", "win.pin-toggle")
        if st["pin"] and n == 1 else None,
        item("Compress to ZIP file", "win.compress") if st["compress"] else None,
        item("Extract all…", "win.extract") if st["extract"] else None,
        item("Open in Terminal", "win.terminal-sel")
        if n == 1 and one.get("is_dir") and one.get("local") else None,
    )
    s5 = section(item("Properties", "win.properties"))
    return menu(s1, s2, s3, s4, s5)


def background_menu(kind, can_paste=False, location=None, pinned=False):
    """Context menu for the empty area of a folder / trash / search view."""
    view = Gio.Menu()
    view.append_submenu("View", view_menu())
    view.append_submenu("Sort by", sort_menu())
    view.append_item(item("Refresh", "win.refresh"))
    if kind == "trash":
        return menu(view, section(item("Empty trash", "win.empty-trash")),
                    section(item("Select all", "win.select-all")))
    s2 = Gio.Menu()
    if kind == "folder":
        s2.append_submenu("New", new_menu())
        s2.append_item(item("Paste", "win.paste"))
    s3 = section(
        item("Open in Terminal", "win.terminal") if kind == "folder" else None,
        item("Unpin from Quick access" if pinned else "Pin to Quick access", "win.pin-toggle")
        if kind == "folder" else None,
        item("Select all", "win.select-all"),
    )
    s4 = section(item("Properties", "win.properties-at", location)
                 if location and kind == "folder" else None)
    return menu(view, s2, s3, s4)


def path_menu(location, pinned, can_unpin=True):
    """Menu for a folder tile on Home / This PC or a sidebar pin."""
    s2 = section(
        item("Unpin from Quick access", "win.unpin", location) if pinned and can_unpin else None,
        item("Pin to Quick access", "win.pin-at", location) if not pinned else None,
        item("Open in Terminal", "win.terminal-at", location),
        item("Copy as path", "win.copy-path-at", location),
    )
    return menu(section(item("Open", "win.open-location", location),
                        item("Open in new tab", "win.open-location-tab", location)),
                s2, section(item("Properties", "win.properties-at", location)))


def recent_menu(uri):
    return menu(section(item("Open", "win.open-uri", uri),
                        item("Open file location", "win.open-uri-location", uri)),
                section(item("Copy as path", "win.copy-path-at", uri),
                        item("Remove from recent", "win.forget-recent", uri)),
                section(item("Properties", "win.properties-at", uri)))
