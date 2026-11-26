"""Guard: preview must not use Gtk.MediaFile (GTK's GStreamer backend aborts
Harbor Files on every MP3 with GStreamer 1.28: decodebin3 assertion)."""

import pathlib

SRC = pathlib.Path(__file__).resolve().parent.parent / "src" / "harbor"


def test_no_gtk_mediafile_in_files_modules():
    for p in SRC.glob("files*.py"):
        if p.name == "files_media.py":
            continue
        assert "Gtk.MediaFile.new" not in p.read_text(), p.name
