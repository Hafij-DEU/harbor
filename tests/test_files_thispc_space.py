"""0.5.41: This PC drive cards keep their free space up to date."""
from gi.repository import Gio

from harbor import files_pages as fp


def _page(space):
    page = fp.ThisPCPage.__new__(fp.ThisPCPage)   # no window needed for the space logic
    page._space = space
    page.calls = []
    page._fill_space = lambda g, *_: page.calls.append(g.get_uri())
    return page


def test_timer_tick_rereads_local_drives_only():
    local = Gio.File.new_for_path("/")
    remote = Gio.File.new_for_uri("ftp://harbor@192.168.1.5:2121/")
    page = _page([(local, None, None, None), (remote, None, None, None)])
    page.update_space(local_only=True)
    assert page.calls == ["file:///"]


def test_page_show_rereads_every_drive():
    local = Gio.File.new_for_path("/")
    remote = Gio.File.new_for_uri("ftp://harbor@192.168.1.5:2121/")
    page = _page([(local, None, None, None), (remote, None, None, None)])
    page.update_space()
    assert page.calls == ["file:///", "ftp://harbor@192.168.1.5:2121/"]


def test_poll_interval_is_short():
    assert 1 <= fp.SPACE_POLL_SECS <= 10
