"""Answer only explicitly requested, PID-owned open-volume warning dialogs.

This is diagnostic input, not an implementation of native open-volume math.
Window titles, process ownership and the transient parent are all checked;
unrelated dialogs and existing user Rhino windows never receive input.
"""
import re
import subprocess
import time

from .area_centroid_probe import validate
from .client import OracleError, OracleProtocolError, _read_optional_text


DIALOG_TITLE = "Rhino 8  Volume properties of non-Closed"
KEYS = {"yes": "alt+y", "no": "alt+n", "escape": "Escape"}
_MARKER = re.compile(r"^VOLUME_CONFIRM ([A-Za-z0-9_.-]{1,100}) (yes|no|escape)$", re.MULTILINE)
_DONE = re.compile(r"^VOLUME_CONFIRM_DONE ([A-Za-z0-9_.-]{1,100})$", re.MULTILINE)


def _query(arguments):
    try:
        result = subprocess.run(arguments, text=True, stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, check=False, timeout=5)
    except (OSError, subprocess.SubprocessError) as error:
        raise OracleError("cannot inspect owned volume dialog: %s" % error) from error
    return result.stdout if result.returncode == 0 else ""


def _properties(window):
    result = _query(["xprop", "-id", window, "_NET_WM_PID", "WM_TRANSIENT_FOR", "WM_NAME"])
    pid = re.search(r"^_NET_WM_PID\(CARDINAL\) = ([0-9]+)$", result, re.MULTILINE)
    parent = re.search(r"^WM_TRANSIENT_FOR\(WINDOW\): window id # (0x[0-9a-fA-F]+)$", result, re.MULTILINE)
    title = re.search(r'^WM_NAME\(STRING\) = "([^"\n]*)"$', result, re.MULTILINE)
    return (int(pid.group(1)) if pid else None,
            parent.group(1) if parent else None,
            title.group(1) if title else None)


def _owned_dialog(owned_pids):
    if not owned_pids:
        return None
    candidates = []
    for line in _query(["wmctrl", "-lp"]).splitlines():
        fields = line.split(maxsplit=4)
        if (len(fields) != 5 or fields[4] != DIALOG_TITLE
                or not re.fullmatch(r"0x[0-9a-fA-F]+", fields[0])
                or not fields[2].isdigit() or int(fields[2]) not in owned_pids):
            continue
        window, pid = fields[0], int(fields[2])
        actual_pid, parent, title = _properties(window)
        if actual_pid != pid or title != DIALOG_TITLE or parent is None:
            continue
        parent_pid, _, parent_title = _properties(parent)
        if (parent_pid != pid or not parent_title or "Rhino 8" not in parent_title
                or parent_title == DIALOG_TITLE or int(parent, 16) == int(window, 16)):
            continue
        candidates.append(dict(window=window, owner_pid=pid, parent_window=parent, title=title))
    if len(candidates) > 1:
        raise OracleError("ambiguous owned open-volume warning dialogs")
    return candidates[0] if candidates else None


class VolumeConfirmation:
    def __init__(self, request):
        operations = request.get("operations")
        if (type(request.get("protocol_version")) is not int or request["protocol_version"] != 1
                or type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1
                or not isinstance(operations, list) or not 1 <= len(operations) <= 128):
            raise OracleProtocolError("volume confirmation requires protocol 1, one iteration and 1 to 128 cases")
        self.ids = []
        self.choices = {}
        for operation in operations:
            try:
                validate(operation, "volume")
            except ValueError as error:
                raise OracleProtocolError("volume confirmation requires a dedicated volume-centroid request") from error
            name = operation["id"]
            if name in self.ids:
                raise OracleProtocolError("duplicated volume confirmation id")
            self.ids.append(name)
            if "open_confirmation" in operation:
                self.choices[name] = operation["open_confirmation"]
        self.delivered = {}
        self.ready = {}

    def __call__(self, job, owned_pids):
        progress = _read_optional_text(job / "worker-progress.log")
        done = set(_DONE.findall(progress))
        pending = [(name, choice) for name, choice in _MARKER.findall(progress)
                   if name not in done and name not in self.delivered]
        if not pending or not owned_pids:
            return
        if len(pending) != 1 or self.choices.get(pending[0][0]) != pending[0][1]:
            raise OracleError("unrequested or ambiguous volume confirmation marker")
        name, choice = pending[0]
        dialog = _owned_dialog(owned_pids)
        if dialog is None:
            return
        # Wine exposes the X11 dialog before its controls reliably accept keys.
        # Let that particular dialog settle without blocking the host loop;
        # ownership/title/parent are inspected again on every subsequent poll.
        ready = self.ready.setdefault((name, dialog["window"]), time.monotonic() + 0.5)
        if time.monotonic() < ready:
            return
        # A closed-object command may finish without ever displaying a warning.
        # Do not send a stale choice to a later command's dialog.
        if name in _DONE.findall(_read_optional_text(job / "worker-progress.log")):
            return
        window = dialog["window"]
        try:
            # Activate the verified dialog, then use normal focused input.
            # XSendEvent (--window) can fail on key-up after Escape destroys
            # the dialog on key-down; it also bypasses ordinary Wine dispatch.
            subprocess.run(["xdotool", "windowactivate", "--sync", window,
                            "key", "--clearmodifiers", KEYS[choice]],
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=10)
        except (OSError, subprocess.SubprocessError) as error:
            raise OracleError("cannot answer owned open-volume dialog: %s" % error) from error
        self.delivered[name] = dialog

    def record_diagnostics(self, response):
        if self.ids != [row["id"] for row in response["results"]]:
            raise OracleProtocolError("volume confirmation result order changed")
        for row in response["results"]:
            name = row["id"]
            if name in self.choices:
                row["value"]["open_confirmation"] = dict(
                    requested=self.choices[name], dialog=self.delivered.get(name))
