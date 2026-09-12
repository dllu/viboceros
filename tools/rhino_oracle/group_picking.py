"""Bounded, real idle-viewport clicks in a newly owned Rhino window."""
import json
import os
import re
import subprocess
import time

from .client import OracleError, OracleProtocolError, _read_optional_text, _rhino_window_for_pids


def validate_request(request):
    operations = request.get("operations")
    if type(request.get("protocol_version")) is not int or request.get("protocol_version") != 1 or type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
        raise OracleProtocolError("group picking requires protocol 1 and one iteration")
    if not isinstance(operations, list) or not 1 <= len(operations) <= 128:
        raise OracleProtocolError("group picking requires 1 to 128 cases")
    seen = set()
    for operation in operations:
        if operation.get("op") not in ("group_picking", "mesh_split_picking", "mesh_explode_picking"):
            raise OracleProtocolError("group picking requires a dedicated request")
        if operation.get("op") in ("mesh_split_picking", "mesh_explode_picking") and any(
            operation.get(field) for field in ("move", "recall_previous", "recall_last", "last_steps", "add_to_group_sources")
        ):
            raise OracleProtocolError("mesh split picking cannot combine other commands")
        name = operation.get("id")
        if not isinstance(name, str) or re.fullmatch(r"[A-Za-z0-9_-]{1,100}", name) is None or name in seen:
            raise OracleProtocolError("invalid or duplicated group picking id")
        seen.add(name)
        def indices(values):
            if not isinstance(values, list) or any(type(i) is not int or not 0 <= i < 3 for i in values) or len(set(values)) != len(values):
                raise OracleProtocolError("invalid group picking object indices")
        indices([operation.get("seed")])
        groups = operation.get("groups")
        if not isinstance(groups, list) or len(groups) > 16:
            raise OracleProtocolError("invalid group picking definitions")
        for members in groups: indices(members)
        sources = operation.get('add_to_group_sources')
        if sources is not None:
            indices(sources)
            if (not sources or not any(operation['seed'] in group for group in groups)
                or any(operation.get(field) for field in ('move', 'recall_previous', 'recall_last', 'last_steps', 'hidden', 'locked', 'layer_mode'))):
                raise OracleProtocolError('invalid AddToGroup picking case')
        for field in ("locked", "hidden"): indices(operation.get(field, []))
        if set(operation.get("locked", [])).intersection(operation.get("hidden", [])):
            raise OracleProtocolError("object mode cannot be both hidden and locked")
        if type(operation.get("reverse_bridge", False)) is not bool or operation.get("layer_mode") not in (None, "locked", "hidden"):
            raise OracleProtocolError("invalid group picking mode")
        if type(operation.get("move", False)) is not bool:
            raise OracleProtocolError("invalid group picking move flag")
        if type(operation.get("recall_previous", False)) is not bool:
            raise OracleProtocolError("invalid group picking recall flag")
        if type(operation.get("recall_last", False)) is not bool:
            raise OracleProtocolError("invalid last-selection recall flag")
        if operation.get("recall_last") and (not operation.get("move") or operation.get("recall_previous")
            or operation['seed'] in operation.get('hidden', []) or operation['seed'] in operation.get('locked', [])
            or (operation['seed'] == 1 and operation.get('layer_mode'))):
            raise OracleProtocolError("last-selection recall requires a completed Move")
        steps = operation.get('last_steps', [])
        if not isinstance(steps, list) or len(steps) > 32 or (steps and not operation.get('recall_last')):
            raise OracleProtocolError('invalid last-selection steps')
        for step in steps:
            if not isinstance(step, dict) or step.get('kind') not in ('select', 'recall', 'undo', 'redo', 'delete', 'layer'):
                raise OracleProtocolError('invalid last-selection step')
            if step['kind'] == 'select': indices(step.get('objects'))
            if step['kind'] == 'recall' and step.get('deselect_others') is not None and type(step['deselect_others']) is not bool:
                raise OracleProtocolError('invalid last-selection option')


class IdlePicker:
    def __init__(self):
        self.seen = set()
        self.ready = {}

    def __call__(self, job, owned_pids):
        for name, x, y in re.findall(r"^PICK (\S+) (\d+) (\d+)$", _read_optional_text(job / "worker-progress.log"), re.MULTILINE):
            if name in self.seen:
                continue
            # Let the viewport redraw and return to Rhino's normal event loop.
            ready = self.ready.setdefault(name, time.monotonic() + 0.5)
            if time.monotonic() < ready or not owned_pids:
                continue
            window = _rhino_window_for_pids(owned_pids)
            if window is None:
                continue
            try:
                # X11 processes the warp before the following click. Waiting for
                # a motion event can stall repeated picks at the same location.
                subprocess.run(["xdotool", "windowactivate", "--sync", window,
                                "mousemove", x, y, "click", "1"], check=True, timeout=10)
            except subprocess.SubprocessError as error:
                progress = _read_optional_text(job / "worker-progress.log")[-2000:]
                raise OracleError("mouse input failed for %s in owned window %s: %s\n%s" %
                                  (name, window, error, progress)) from error
            # Acknowledgement permits observation, never another command/Enter.
            temporary = job / "click-ack.json.tmp"
            temporary.write_text(json.dumps(name), encoding="utf-8")
            os.replace(temporary, job / "click-ack.json")
            self.seen.add(name)
