"""Final mouse, group and selection-history picker setup."""
from pathlib import Path
import subprocess
import time


def prepare(client, request, interaction, worker_source):
    if any(op.get("op") == "point_snap" for op in request.get("operations", [])):
        from .point_snap_input import PointSnapPicker
        interaction = PointSnapPicker(request)
    if any(op.get("op") == "angle_cursor_diagnostic" or
           (op.get("op") == "plane_primitive" and op.get("primitive") in
            ("ArcCenterEndpoint", "ArcStartCenterEndpoint", "ArcMidpointEndpoint", "ArcDefaultEndpoint"))
           for op in request.get("operations", [])):
        from .group_picking import IdlePicker
        if any(op.get("op") == "angle_cursor_diagnostic" for op in request.get("operations", [])):
            from .angle_cursor_probe import validate_request
            validate_request(request)
        class AnglePicker(IdlePicker):
            def send_input(self, name, x, y, window):
                # The worker announces the target before RunScript reaches _Pause.
                time.sleep(1.0)
                subprocess.run(["xdotool", "windowactivate", "--sync", window,
                                "mousemove", str(int(x) + 1), y,
                                "mousemove", x, y], check=True, timeout=10)
                time.sleep(0.5)
                subprocess.run(["xdotool", "windowactivate", "--sync", window,
                                "click", "1"], check=True, timeout=10)
                return True
        interaction = AnglePicker()
    if any(op.get("op") in ("merge_edge_command", "split_edge_command") and op.get("pick") == "mouse"
           for op in request.get("operations", [])):
        from .group_picking import IdlePicker
        from .merge_edges_probe import validate_mouse_request
        validate_mouse_request(request)
        interaction = IdlePicker()
    if any(op.get("op") in ("mesh_connected_command_probe", "mesh_part_command_probe")
           and op.get("mouse_pick")
           for op in request.get("operations", [])):
        from .group_picking import IdlePicker
        accept = []
        pause = []
        for op in request["operations"]:
            if op.get("op") != "mesh_part_command_probe":
                continue
            count = len(op.get("pick_points", [op.get("pick_point")]))
            names = ([op["id"]] if count == 1 else
                     ["%s-%d" % (op["id"], index + 1) for index in range(count)])
            pause.extend(names)
            accept.append(names[-1])
        interaction = IdlePicker(accept_after_click=accept,
                                 pause_after_click=pause)
    if any(operation.get("op") in ("group_picking", "mesh_split_picking", "mesh_explode_picking") for operation in request.get("operations", [])):
        from .group_picking import IdlePicker, validate_request
        validate_request(request)
        worker_source = Path(__file__).with_name("group_picking_worker.py")
        interaction = IdlePicker()
    if any(operation.get("op") == "undo_selection" for operation in request.get("operations", [])):
        from .undo_selection import validate_request
        validate_request(request)
        worker_source = Path(__file__).with_name("undo_selection_worker.py")
    return interaction, worker_source
