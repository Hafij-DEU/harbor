"""Wi-Fi power saving check before sharing with a phone.

Power saving on the PC's Wi-Fi card makes every round trip to the phone
wait, so phone folders open slowly. Each time the user opens
Share over Wi-Fi or scans a phone, Harbor checks the setting and, while
it is on, explains what happens and offers to turn it off. Skipping is
fine; the question comes back next time. The switch in
Settings > Behaviour changes it too.
"""
from gi.repository import Adw

from . import common
from . import files_core as fc

HEADING = "Turn off Wi-Fi power saving?"
BODY = (
    "Wi-Fi power saving is on for this PC. While it is on, the Wi-Fi card sleeps "
    "between packets, and every request to your phone can wait from a fraction "
    "of a second up to a few seconds.\n\n"
    "If you keep it on:\n"
    "• Phone folders open slowly. A big folder such as Camera, with hundreds of "
    "photos, can take 10–20 seconds or more.\n"
    "• Copying files from the phone is slower, and thumbnails appear late.\n"
    "• The phone connection can stall or time out, and Harbor may show the phone "
    "as disconnected.\n"
    "• Sending files from the PC to the phone is affected less, but can also slow down.\n\n"
    "Turning it off uses a little more battery on this PC while Wi-Fi is on. "
    "It changes the setting for your saved Wi-Fi networks and takes effect the "
    "next time Wi-Fi connects (turn Wi-Fi off and on to apply it now).\n\n"
    "Harbor asks again each time while it is on. You can also change it in Settings › Behaviour › Wi-Fi power saving."
)


def ask_if_on(win, then):
    """Run then() after the question, or straight away when power saving is off."""
    def got(ok, out, _err):
        state = fc.wifi_ps_state(out if ok else "")
        if fc.wifi_ps_saved_on(state) is not True:
            then()
            return
        _show(win, list(state["conns"]), then)
    common.run_cmd(fc.wifi_ps_query_argv(), got, mutating=False)


def _show(win, conns, then):
    dlg = Adw.AlertDialog(heading=HEADING, body=BODY)
    dlg.add_response("skip", "Keep it on")
    dlg.add_response("off", "Turn off power saving")
    dlg.set_response_appearance("off", Adw.ResponseAppearance.SUGGESTED)
    dlg.set_default_response("off")
    dlg.set_close_response("skip")

    def response(_d, rid):
        if rid != "off":
            win.notify_toast("Wi-Fi power saving stays on: phone folders may be slow", timeout=5)
            then()
            return

        def done(ok, _out, err):
            if common.DRY_RUN:
                win.notify_toast("Dry run: would turn Wi-Fi power saving off")
            elif ok:
                win.notify_toast("Wi-Fi power saving off. Applies when Wi-Fi reconnects.", timeout=5)
            else:
                win.notify_toast("Can't change Wi-Fi power saving: %s" % (err.strip() or "error"),
                                 timeout=5)
            then()
        common.run_cmd(fc.wifi_ps_set_argv(conns, False), done)
    dlg.connect("response", response)
    dlg.present(win)
