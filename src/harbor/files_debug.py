"""Harbor Files: main-loop stall watchdog (HARBOR_DEBUG_STALLS=1).

A GLib timeout on the main loop bumps a heartbeat every 25 ms. A watcher
thread checks it; when the heartbeat is older than the threshold it grabs
the main thread's Python stack (sys._current_frames) while the stall is
still going on, so the log shows what is blocking. Every gap measured on
the main loop is also recorded, so a run can report its worst stall.

Env:
  HARBOR_DEBUG_STALLS=1         turn it on
  HARBOR_STALL_MS=150           threshold in milliseconds
  HARBOR_STALL_LOG=/path/file   append the log there (default: stderr)
"""

import os
import sys
import threading
import time
import traceback

from gi.repository import GLib

_state = None


def enabled():
    return os.environ.get("HARBOR_DEBUG_STALLS", "") not in ("", "0")


class Watchdog:
    def __init__(self, threshold_ms=150, log_path=None, tick_ms=25):
        self.threshold = threshold_ms / 1000.0
        self.tick_ms = tick_ms
        self.log_path = log_path
        self.main_ident = threading.get_ident()
        self.beat = time.monotonic()
        self.last_tick = self.beat
        self.worst = 0.0
        self.worst_stack = ""
        self.stalls = []          # (gap seconds, label)
        self.label = ""           # what the app says it is doing (optional)
        self._sampled = None      # (beat it belongs to, stack text)
        self._stop = threading.Event()
        self._lock = threading.Lock()

    # ------------------------------------------------------------ main loop side

    def start(self):
        GLib.timeout_add(self.tick_ms, self._tick)
        threading.Thread(target=self._watch, daemon=True, name="hb-stall-watch").start()

    def _tick(self):
        now = time.monotonic()
        gap = now - self.last_tick - self.tick_ms / 1000.0
        self.last_tick = now
        with self._lock:
            self.beat = now
            sampled = self._sampled
            self._sampled = None
        if gap > self.threshold:
            stack = sampled[1] if sampled else "(no stack sample)"
            self.stalls.append((gap, self.label))
            if gap > self.worst:
                self.worst, self.worst_stack = gap, stack
            self._log("STALL %.0f ms%s\n%s" % (gap * 1000, (" [%s]" % self.label) if self.label
                                                else "", stack))
        return not self._stop.is_set()

    # ------------------------------------------------------------ watcher thread

    def _watch(self):
        while not self._stop.wait(self.threshold / 2):
            with self._lock:
                beat = self.beat
                have = self._sampled is not None and self._sampled[0] == beat
            if have or time.monotonic() - beat < self.threshold:
                continue
            frame = sys._current_frames().get(self.main_ident)
            text = "".join(traceback.format_stack(frame)[-14:]) if frame else "(main idle in C)"
            with self._lock:
                if self.beat == beat:
                    self._sampled = (beat, text)

    # ------------------------------------------------------------ output

    def _log(self, text):
        line = "[harbor-stall %s] %s\n" % (time.strftime("%H:%M:%S"), text)
        if self.log_path:
            try:
                with open(self.log_path, "a", encoding="utf-8") as f:
                    f.write(line)
                return
            except OSError:
                pass
        sys.stderr.write(line)

    def summary(self):
        n = len(self.stalls)
        return "stalls>%d ms: %d, worst %.0f ms" % (self.threshold * 1000, n, self.worst * 1000)

    def stop(self):
        self._stop.set()
        self._log("SUMMARY " + self.summary())


def start_if_enabled():
    """Start the watchdog once when HARBOR_DEBUG_STALLS is set; returns it or None."""
    global _state
    if _state is not None or not enabled():
        return _state
    try:
        ms = int(os.environ.get("HARBOR_STALL_MS", "150"))
    except ValueError:
        ms = 150
    _state = Watchdog(ms, os.environ.get("HARBOR_STALL_LOG") or None)
    _state.start()
    return _state


def get():
    return _state
