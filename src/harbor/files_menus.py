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
                        item("Markdown document", "win.new-doc", "markdown",
                             icon="text-x-generic-symbolic"),
                        item("Word document", "win.new-doc", "docx",
                             icon="x-office-document-symbolic"),
                        item("Spreadsheet", "win.new-doc", "xlsx",
                             icon="x-office-spreadsheet-symbolic"),
                        item("Presentation", "win.new-doc", "pptx",
                             icon="x-office-presentation-symbolic"),
                        item("Empty file", "win.new-empty", icon="document-new-symbolic")))


def sevenzip_menu(sel):
    """7-Zip submenu for the selection (sel as in fc.command_states)."""
    n = len(sel)
    one = sel[0] if n == 1 else {}
    arch = n == 1 and bool(one.get("archive"))
    stem = fa_stem(one.get("name", "")) if arch else ""
    files = [s for s in sel if not s.get("is_dir")]
    m = menu(
        section(item("Open archive", "win.7z", "open") if arch else None,
                item("Extract files…", "win.7z", "extract-dialog") if arch else None,
                item("Extract here", "win.7z", "extract-here") if arch else None,
                item("Extract to “%s/”" % stem, "win.7z", "extract-to") if arch else None,
                item("Test archive", "win.7z", "test") if arch else None),
        section(item("Add to archive…", "win.7z", "add-dialog"),
                item("Add to “%s.7z”" % add_stem(sel), "win.7z", "add-7z"),
                item("Add to “%s.zip”" % add_stem(sel), "win.7z", "add-zip")),
        section(item("CRC SHA", "win.7z", "hash") if files else None))
    return m


def fa_stem(name):
    from . import files_archive
    return files_archive.archive_stem(name)


def add_stem(sel):
    if len(sel) == 1:
        name = sel[0].get("name", "") or "Archive"
        if not sel[0].get("is_dir"):
            name = name.rsplit(".", 1)[0] if "." in name[1:] else name
        return name
    return sel[0].get("parent", "") or "Archive" if sel else "Archive"


def extra_section(sel, st):
    """Duplicate, Copy name, Make executable, Set as wallpaper, Customize folder."""
    n = len(sel)
    one = sel[0] if n == 1 else {}
    ct = (one.get("content_type") or "")
    return section(
        item("Duplicate", "win.duplicate", icon="edit-copy-symbolic") if st["copy"] else None,
        item("Copy name", "win.copy-name") if n else None,
        item("Make executable", "win.make-executable")
        if st["copy"] and any(not s.get("is_dir") for s in sel) else None,
        item("Set as desktop background", "win.set-wallpaper",
             icon="preferences-desktop-wallpaper-symbolic")
        if n == 1 and one.get("local") and ct.startswith("image/") else None,
        item("Customize folder…", "win.customize-folder", icon="folder-symbolic")
        if n == 1 and one.get("is_dir") and one.get("local") else None,
        item("Share over Wi-Fi…", "win.share-folder", icon="network-wireless-symbolic")
        if n and all(s.get("local") for s in sel) else None,
    )


def sort_menu():
    return menu(section(*[item(lab, "win.sort", key) for key, lab in SORT_LABELS]),
                section(item("Ascending", "win.sort-order", "asc"),
                        item("Descending", "win.sort-order", "desc")))


def view_menu():
    return menu(section(*[item(lab, "win.view", key) for key, lab in VIEW_LABELS]),
                section(item("Navigation pane", "win.nav-pane"),
                        item("Preview pane", "win.preview-pane"),
                        item("Details pane", "win.details-pane"),
                        item("Hidden items", "win.show-hidden"),
                        item("File name extensions", "win.show-ext")))


def more_menu(pin_label="Pin to Quick access"):
    return menu(section(item("Select all", "win.select-all"),
                        item("Select none", "win.select-none"),
                        item("Invert selection", "win.invert-selection")),
                section(item(pin_label, "win.pin-toggle"),
                        item("Open in Terminal", "win.terminal"),
                        item("Copy path", "win.copy-path")),
                section(item("Add network location…", "win.add-network"),
                        item("Share over Wi-Fi…", "win.share-wifi"),
                        item("Scan phone QR…", "win.scan-phone")),
                section(item("Undo", "win.undo")),
                section(item("Properties", "win.properties"),
                        item("Settings", "win.options"),
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


# ---------------------------------------------------------------------- Windows 11 / classic menus

def verb(label, action, icon, target=None):
    """Menu item shown as an icon button in a "horizontal-buttons" section."""
    mi = item(label, action, target)
    mi.set_attribute_value("verb-icon", GLib.Variant("s", icon))
    return mi


def button_row(*items):
    """Section rendered as a row of icon buttons (Windows 11 menu top row)."""
    sec = Gio.MenuItem.new_section(None, section(*items))
    sec.set_attribute_value("display-hint", GLib.Variant("s", "horizontal-buttons"))
    return sec


def _with_row(row, *sections):
    m = Gio.Menu()
    if row is not None:
        m.append_item(row)
    for s in sections:
        if s is not None and s.get_n_items():
            m.append_section(None, s)
    return m


def more_options_section():
    return section(item("Show more options", "win.more-options",
                        icon="view-more-horizontal-symbolic"))


def win11_item_menu(kind, sel, can_paste=False, pinned=False):
    """Windows 11 item menu: icon row, the common commands, Show more options."""
    if kind == "trash":
        return item_menu(kind, sel, can_paste, pinned)
    st = fc.command_states(kind, sel, can_paste=can_paste)
    n = len(sel)
    one = sel[0] if n == 1 else {}
    row = button_row(
        verb("Cut", "win.cut", "edit-cut-symbolic") if st["cut"] else None,
        verb("Copy", "win.copy", "edit-copy-symbolic") if st["copy"] else None,
        verb("Rename", "win.rename", "document-edit-symbolic") if st["rename"] else None,
        verb("Copy path", "win.copy-path", "insert-link-symbolic") if st["copy-path"] else None,
        verb("Delete", "win.delete", "user-trash-symbolic") if st["delete"] else None,
    )
    s1 = section(item("Open", "win.open", icon="document-open-symbolic"))
    if st["open-with"]:
        s1.append_submenu("Open with", open_with_menu(one.get("content_type", "")))
    s1b = section(
        item("Open in new tab", "win.open-tab", icon="tab-new-symbolic")
        if st["open-new-tab"] else None,
        item("Open in new window", "win.open-window", icon="window-new-symbolic")
        if st["open-new-tab"] else None,
        item("Open file location", "win.open-location-of") if kind == "search" else None,
    )
    s2 = section(
        item("Compress to ZIP file", "win.compress", icon="package-x-generic-symbolic")
        if st["compress"] else None,
        item("Extract all…", "win.extract") if st["extract"] else None,
        item("Copy as path", "win.copy-path") if st["copy-path"] else None,
        item("Unpin from Quick access" if pinned else "Pin to Quick access", "win.pin-toggle",
             icon="starred-symbolic") if st["pin"] and n == 1 else None,
        item("Paste into folder", "win.paste-into", icon="edit-paste-symbolic")
        if n == 1 and one.get("is_dir") and one.get("local") and can_paste else None,
        item("Properties", "win.properties", icon="document-properties-symbolic"),
    )
    s3 = extra_section(sel, st)
    if st["copy"]:
        s3.append_submenu("7-Zip", sevenzip_menu(sel))
    return _with_row(row, s1, s1b, s2, s3, more_options_section())


def win11_background_menu(kind, can_paste=False, location=None, pinned=False):
    m = background_menu(kind, can_paste, location, pinned)
    if kind == "folder":
        m.append_section(None, more_options_section())
    return m


def send_to_menu(targets):
    """Send to submenu from fc.send_to_targets()."""
    m = Gio.Menu()
    for label, kind, dest in targets:
        m.append_item(item(label, "win.send-to", kind if dest is None else "%s:%s" % (kind, dest)))
    return m


def classic_item_menu(kind, sel, can_paste=False, pinned=False, send_to=()):
    """Windows 10 style item menu (Show more options / Shift+F10)."""
    if kind == "trash":
        return item_menu(kind, sel, can_paste, pinned)
    st = fc.command_states(kind, sel, can_paste=can_paste)
    n = len(sel)
    one = sel[0] if n == 1 else {}
    folder_one = n == 1 and one.get("is_dir") and one.get("local")
    s1 = section(
        item("Open", "win.open"),
        item("Open in new window", "win.open-window") if st["open-new-tab"] else None,
        item("Open file location", "win.open-location-of") if kind == "search" else None,
        item("Unpin from Quick access" if pinned else "Pin to Quick access", "win.pin-toggle")
        if st["pin"] and n == 1 else None,
    )
    if st["open-with"]:
        s1.append_submenu("Open with", open_with_menu(one.get("content_type", "")))
    if folder_one:
        s1.append_item(item("Open in Terminal", "win.terminal-sel"))
    s2 = Gio.Menu()
    if st["copy"] and send_to:
        s2.append_submenu("Send to", send_to_menu(send_to))
    s3 = section(
        item("Cut", "win.cut") if st["cut"] else None,
        item("Copy", "win.copy") if st["copy"] else None,
        item("Paste", "win.paste-into") if folder_one and can_paste else None,
    )
    s4 = section(
        item("Create shortcut", "win.create-shortcut") if st["copy"] else None,
        item("Delete", "win.delete") if st["delete"] else None,
        item("Rename", "win.rename") if st["rename"] else None,
    )
    if st["copy"]:
        s2.append_submenu("7-Zip", sevenzip_menu(sel))
    s4b = extra_section(sel, st)
    s5 = section(item("Properties", "win.properties"))
    return menu(s1, s2, s3, s4, s4b, s5)


def classic_background_menu(kind, can_paste=False, location=None, pinned=False):
    """Windows 10 style menu for the empty area of a folder."""
    if kind != "folder":
        return background_menu(kind, can_paste, location, pinned)
    s1 = Gio.Menu()
    s1.append_submenu("View", view_menu())
    s1.append_submenu("Sort by", sort_menu())
    s1.append_item(item("Refresh", "win.refresh"))
    s2 = section(item("Paste", "win.paste"), item("Paste shortcut", "win.paste-shortcut"))
    s3 = section(item("Open in Terminal", "win.terminal"))
    s4 = Gio.Menu()
    s4.append_submenu("New", new_menu())
    s5 = section(item("Properties", "win.properties-at", location) if location else None)
    return menu(s1, s2, s3, s4, s5)


def location_menu(action, places):
    """Move to / Copy to submenu: Quick access folders + Choose location…"""
    m = Gio.Menu()
    m.append_section(None, section(*[item(label, "win." + action, path,
                                          icon=icon) for label, path, icon in places]))
    m.append_section(None, section(item("Choose location…", "win.%s-choose" % action)))
    return m


def actions_in(model):
    """Every action name (without "win.") used in a menu model, submenus included."""
    out = set()

    def walk(m):
        for i in range(m.get_n_items()):
            a = m.get_item_attribute_value(i, "action", GLib.VariantType.new("s"))
            if a is not None:
                out.add(a.unpack().split(".", 1)[-1])
            for link in ("section", "submenu"):
                sub = m.get_item_link(i, link)
                if sub is not None:
                    walk(sub)
    walk(model)
    return out
