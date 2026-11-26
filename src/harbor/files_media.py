"""Harbor Files: audio/video player for the preview pane.

Gtk.MediaFile is NOT used. GTK's GStreamer media backend (GstPlay over
playbin3) aborts the whole process on every MP3 with GStreamer 1.28.2:
"gstdecodebin3.c: mq_slot_handle_stream_start: assertion failed:
(collection)". A plain playbin driven from here does not hit that race, so
this module wraps one in a Gtk.MediaStream. Gtk.MediaControls works with it
unchanged; video frames come from gtk4paintablesink when it is installed.
"""

import gi

try:
    gi.require_version("Gst", "1.0")
    from gi.repository import Gst
except (ValueError, ImportError):  # GStreamer typelib missing
    Gst = None

from gi.repository import GLib, Gtk

POS_POLL_MS = 250

_inited = False


def available():
    """True when GStreamer and the playbin element can be used."""
    global _inited
    if Gst is None:
        return False
    if not _inited:
        Gst.init(None)
        _inited = True
    return Gst.ElementFactory.find("playbin") is not None


class PlaybinStream(Gtk.MediaStream):
    """A Gtk.MediaStream over one playbin. Call close() when done with it."""

    def __init__(self, uri, video=False):
        super().__init__()
        self.paintable = None
        self._timer = 0
        self._prepared = False
        self._seeking = False
        self._closed = False
        self.pipe = Gst.ElementFactory.make("playbin", None)
        self.pipe.set_property("uri", uri)
        sink = None
        if video:
            sink = Gst.ElementFactory.make("gtk4paintablesink", None)
            if sink is not None:
                self.paintable = sink.get_property("paintable")
        if sink is None:
            sink = Gst.ElementFactory.make("fakesink", None)
        self.pipe.set_property("video-sink", sink)
        bus = self.pipe.get_bus()
        bus.add_signal_watch()
        self._bus_id = bus.connect("message", self._on_message)
        self.pipe.set_state(Gst.State.PAUSED)

    # ------------------------------------------------------------ Gst side
    def _on_message(self, _bus, msg):
        t = msg.type
        if t == Gst.MessageType.ASYNC_DONE:
            if not self._prepared:
                self._prepared = True
                ok, dur = self.pipe.query_duration(Gst.Format.TIME)
                q = Gst.Query.new_seeking(Gst.Format.TIME)
                seekable = self.pipe.query(q) and q.parse_seeking()[1]
                self.stream_prepared(self.pipe.get_property("n-audio") > 0,
                                     self.pipe.get_property("n-video") > 0,
                                     bool(seekable), dur // 1000 if ok and dur > 0 else 0)
            if self._seeking:
                self._seeking = False
                self.seek_success()
                self._update_pos()
        elif t == Gst.MessageType.EOS:
            self._stop_timer()
            self._update_pos()
            self.stream_ended()
        elif t == Gst.MessageType.ERROR:
            err, _dbg = msg.parse_error()
            self._stop_timer()
            if self._seeking:
                self._seeking = False
                self.seek_failed()
            self.gerror(err)
            self.close()

    def _update_pos(self):
        ok, pos = self.pipe.query_position(Gst.Format.TIME)
        if ok and pos >= 0:
            self.update(pos // 1000)
        return True

    def _stop_timer(self):
        if self._timer:
            GLib.source_remove(self._timer)
            self._timer = 0

    # ------------------------------------------------------------ GtkMediaStream
    def do_play(self):
        if self._closed:
            return False
        self.pipe.set_state(Gst.State.PLAYING)
        if not self._timer:
            self._timer = GLib.timeout_add(POS_POLL_MS, self._update_pos)
        return True

    def do_pause(self):
        self._stop_timer()
        if not self._closed:
            self.pipe.set_state(Gst.State.PAUSED)
            self._update_pos()

    def do_seek(self, timestamp):
        if self._closed:
            self.seek_failed()
            return
        self._seeking = True
        if not self.pipe.seek_simple(Gst.Format.TIME,
                                     Gst.SeekFlags.FLUSH | Gst.SeekFlags.ACCURATE,
                                     timestamp * 1000):
            self._seeking = False
            self.seek_failed()

    def do_update_audio(self, muted, volume):
        if not self._closed:
            self.pipe.set_property("mute", muted)
            self.pipe.set_property("volume", volume)

    def close(self):
        """Stop playback and free the pipeline (safe to call twice)."""
        if self._closed:
            return
        self._closed = True
        self._stop_timer()
        bus = self.pipe.get_bus()
        bus.disconnect(self._bus_id)
        bus.remove_signal_watch()
        self.pipe.set_state(Gst.State.NULL)


def player(path, video):
    """(widget, stream) for the preview pane, or (None, None) if unusable."""
    if not available():
        return None, None
    stream = PlaybinStream(Gst.filename_to_uri(path), video=video)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4, vexpand=video,
                  css_classes=["hbf-preview-video"])
    if video and stream.paintable is not None:
        box.append(Gtk.Picture(paintable=stream.paintable, vexpand=True,
                               content_fit=Gtk.ContentFit.CONTAIN))
    box.append(Gtk.MediaControls(media_stream=stream))
    return box, stream
