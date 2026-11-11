"""Harbor Files: running file operations with UI (progress, conflicts) and the clipboard."""

import os
import threading

from gi.repository import Adw, Gdk, Gio, GLib, Gtk

from . import files_core as fc

COPIED_MIME = "x-special/gnome-copied-files"
URI_MIME = "text/uri-list"


class Operations:
    """Runs copy/move/trash/delete jobs in worker threads for one window.

    The window provides: ops_box (progress area), notify_toast(), and
    refresh_after_op() — called when a job finishes.
    """

    def __init__(self, window):
        self.win = window
        self.jobs = []
        self._tick = None
        self.internal = None   # (cut, [paths]) we put on the clipboard
        self.clipboard = window.get_clipboard()
        self.clipboard.connect("changed", lambda *_: window.update_actions())

    # ------------------------------------------------------------ running

    def _start(self, label, fn, job, on_done=None):
        job.label = label
        self.jobs.append(job)
        self._show_progress()

        def done(res):
            if job in self.jobs:
                self.jobs.remove(job)
            self._show_progress()
            if isinstance(res, Exception):
                job.errors.append(str(res))
            if job.errors:
                more = " (+%d more)" % (len(job.errors) - 1) if len(job.errors) > 1 else ""
                self.win.notify_toast(job.errors[0] + more, timeout=6)
            elif job.cancelled:
                self.win.notify_toast(label + " cancelled")
            if on_done:
                on_done(job)
            self.win.refresh_after_op()
        fc.run_in_thread(lambda: fn(job), done)
        return job

    def _show_progress(self):
        box = self.win.ops_box
        if not self.jobs:
            box.set_visible(False)
            if self._tick:
                GLib.source_remove(self._tick)
                self._tick = None
            return
        box.set_visible(True)
        if self._tick is None:
            self._tick = GLib.timeout_add(150, self._update_progress)
        self._update_progress()

    def _update_progress(self):
        if not self.jobs:
            self._tick = None
            return False
        job = self.jobs[0]
        frac = (job.done / job.total) if job.total else 0.0
        text = job.label
        if len(self.jobs) > 1:
            text += "  (+%d)" % (len(self.jobs) - 1)
        self.win.ops_label.set_label(text)
        if job.total:
            self.win.ops_bar.set_fraction(min(1.0, frac))
        else:
            self.win.ops_bar.pulse()
        return True

    def cancel_all(self):
        for j in list(self.jobs):
            j.cancel()

    # ------------------------------------------------------------ conflict dialog (worker side)

    def _ask_conflict(self, src, dst):
        """Called in the worker thread; blocks it until the user answers."""
        ev = threading.Event()
        answer = {"choice": None, "all": False}
        GLib.idle_add(self._conflict_dialog, src, dst, answer, ev)
        ev.wait()
        return answer["choice"], answer["all"]

    def _conflict_dialog(self, src, dst, answer, ev):
        name = os.path.basename(dst)
        is_dir = os.path.isdir(dst) and os.path.isdir(src)
        dlg = Adw.AlertDialog(
            heading="Replace or skip?",
            body=("The destination already has a %s named “%s”."
                  % ("folder" if is_dir else "file", name)))
        dlg.add_response("cancel", "Cancel")
        dlg.add_response("skip", "Skip")
        dlg.add_response("replace", "Merge" if is_dir else "Replace")
        dlg.add_response("keep", "Keep both")
        dlg.set_response_appearance("keep", Adw.ResponseAppearance.SUGGESTED)
        dlg.set_response_appearance("replace", Adw.ResponseAppearance.DESTRUCTIVE)
        dlg.set_default_response("keep")
        dlg.set_close_response("cancel")
        check = Gtk.CheckButton(label="Do this for all conflicts", halign=Gtk.Align.CENTER)
        dlg.set_extra_child(check)

        def resp(_d, r):
            answer["choice"] = {"skip": fc.SKIP, "replace": fc.REPLACE,
                                "keep": fc.KEEP_BOTH}.get(r)
            answer["all"] = check.get_active()
            ev.set()
        dlg.connect("response", resp)
        dlg.present(self.win)
        return False

    # ------------------------------------------------------------ operations

    def transfer(self, paths, dest_dir, move=False, on_done=None):
        paths = [p for p in paths if p]
        if not paths or not dest_dir:
            return None
        job = fc.Job(ask=self._ask_conflict)
        n = len(paths)
        label = "%s %s" % ("Moving" if move else "Copying", fc.item_count_text(n))
        return self._start(label, lambda j: fc.transfer(j, paths, dest_dir, move), job, on_done)

    def trash(self, paths, on_done=None):
        if not paths:
            return None
        job = fc.Job()
        return self._start("Moving %s to the trash" % fc.item_count_text(len(paths)),
                           lambda j: fc.trash_paths(j, paths), job, on_done)

    def delete_permanently(self, paths, on_done=None):
        if not paths:
            return None
        job = fc.Job()
        return self._start("Deleting %s" % fc.item_count_text(len(paths)),
                           lambda j: fc.delete_permanently(j, paths), job, on_done)

    def delete_gfiles(self, gfiles, label, on_done=None):
        """Permanent delete of non-path files (trash:/// items)."""
        job = fc.Job()

        def run(j):
            j.total = len(gfiles)
            for g in gfiles:
                if j.cancelled:
                    break
                try:
                    _delete_gfile_tree(g, j.cancellable)
                except GLib.Error as e:
                    j.errors.append("%s: %s" % (g.get_basename(), e.message))
                j.done += 1
            return j
        return self._start(label, run, job, on_done)

    def restore(self, items, on_done=None):
        """Move trashed items back to where they came from (never overwrite)."""
        job = fc.Job()

        def run(j):
            j.total = len(items)
            for gfile, orig in items:
                if j.cancelled:
                    break
                try:
                    if not orig:
                        raise GLib.Error("original location unknown")
                    os.makedirs(os.path.dirname(orig), exist_ok=True)
                    target = fc.restore_target(orig)
                    gfile.move(Gio.File.new_for_path(target), Gio.FileCopyFlags.NOFOLLOW_SYMLINKS,
                               j.cancellable, None, None)
                    j.copied.append(target)
                except (GLib.Error, OSError) as e:
                    j.errors.append("%s: %s" % (gfile.get_basename(), getattr(e, "message", e)))
                j.done += 1
            return j
        return self._start("Restoring %s" % fc.item_count_text(len(items)), run, job, on_done)

    # ------------------------------------------------------------ clipboard

    def set_clipboard(self, paths, cut):
        self.internal = (cut, list(paths))
        gnome = fc.gnome_copied_files(paths, cut).encode()
        provs = [
            Gdk.ContentProvider.new_for_bytes(COPIED_MIME, GLib.Bytes.new(gnome)),
            Gdk.ContentProvider.new_for_bytes(URI_MIME, GLib.Bytes.new(fc.uri_list(paths).encode())),
            Gdk.ContentProvider.new_for_bytes("text/plain;charset=utf-8",
                                              GLib.Bytes.new("\n".join(paths).encode())),
        ]
        self.clipboard.set_content(Gdk.ContentProvider.new_union(provs))
        self.win.update_actions()

    def set_text(self, text):
        self.clipboard.set(text)

    def can_paste(self):
        if self.clipboard.is_local():
            return bool(self.internal and self.internal[1])
        formats = self.clipboard.get_formats()
        return bool(formats and (formats.contain_mime_type(COPIED_MIME)
                                 or formats.contain_mime_type(URI_MIME)))

    def paste(self, dest_dir, on_done=None):
        if self.clipboard.is_local() and self.internal:
            cut, paths = self.internal
            self._do_paste(cut, paths, dest_dir, on_done)
            return

        def got(cb, res):
            try:
                stream, mime = cb.read_finish(res)
            except GLib.Error:
                self.win.notify_toast("Nothing to paste")
                return
            out = Gio.MemoryOutputStream.new_resizable()

            def spliced(o, r):
                try:
                    o.splice_finish(r)
                except GLib.Error:
                    return
                data = o.steal_as_bytes().get_data().decode("utf-8", "replace")
                cut, paths = fc.parse_copied_files(data)
                if mime == URI_MIME:
                    cut = False
                self._do_paste(cut, paths, dest_dir, on_done)
            out.splice_async(stream, Gio.OutputStreamSpliceFlags.CLOSE_SOURCE
                             | Gio.OutputStreamSpliceFlags.CLOSE_TARGET,
                             GLib.PRIORITY_DEFAULT, None, spliced)
        self.clipboard.read_async([COPIED_MIME, URI_MIME], GLib.PRIORITY_DEFAULT, None, got)

    def _do_paste(self, cut, paths, dest_dir, on_done):
        paths = [p for p in paths if os.path.lexists(p)]
        if not paths:
            self.win.notify_toast("The copied items no longer exist")
            return

        def finished(job):
            if cut and not job.cancelled:
                self.internal = None
                if self.clipboard.is_local():
                    self.clipboard.set_content(None)
                self.win.update_actions()
            if on_done:
                on_done(job)
        self.transfer(paths, dest_dir, move=cut, on_done=finished)


def _delete_gfile_tree(g, cancellable):
    info = g.query_info("standard::type", Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS, cancellable)
    if info.get_file_type() == Gio.FileType.DIRECTORY and g.get_uri_scheme() != "trash":
        en = g.enumerate_children("standard::name", Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS,
                                  cancellable)
        for ci in en:
            _delete_gfile_tree(en.get_child(ci), cancellable)
    g.delete(cancellable)
