# -*- coding: utf-8 -*-
"""Actual SplitEdge command, with a mouse-selected component and bounded point input."""
import math
import os
from contextlib import contextmanager
if __package__:
    from . import merge_edges_probe
else:
    import merge_edges_probe


def validate(operation):
    if (operation.get("op") != "split_edge_command" or operation.get("pick") != "mouse" or
            any(key in operation for key in ("choice", "cancel", "preselect"))):
        raise ValueError("SplitEdge requires mouse component selection")
    def finite(value):
        try:
            return type(value) in (int, float) and not math.isnan(value) and not math.isinf(value)
        except OverflowError:
            return False
    if type(operation.get("record_viewport", False)) is not bool:
        raise ValueError("record_viewport must be boolean")
    persistent = operation.get("persistent_snaps", [])
    if (not isinstance(persistent, list) or len(persistent) > 6 or
            any(mode not in ("Point", "End", "Mid", "Cen", "Quad", "Near") for mode in persistent) or
            len(set(persistent)) != len(persistent)):
        raise ValueError("invalid persistent snap modes")
    if "inputs" in operation:
        inputs = operation["inputs"]
        def valid_step(step):
            if not isinstance(step, dict) or len(step) != 1:
                return False
            if "pick" not in step:
                return next(iter(step)) in ("point", "mouse", "distance") and finite(next(iter(step.values())))
            pick = step["pick"]
            if (not isinstance(pick, dict) or set(pick) - set(("point", "aim", "osnap", "offset")) or
                    pick.get("osnap") not in ("NoSnap", "Point", "End", "Mid", "Cen", "Quad", "Near", "Persistent")):
                return False
            point, offset = pick.get("point"), pick.get("offset", [0, 0])
            aim = pick.get("aim", point)
            return (isinstance(point, list) and len(point) == 3 and all(finite(v) for v in point) and
                    isinstance(aim, list) and len(aim) == 3 and all(finite(v) for v in aim) and
                    (pick["osnap"] != "Persistent" or bool(persistent)) and
                    isinstance(offset, list) and len(offset) == 2 and
                    all(type(v) is int and -32 <= v <= 32 for v in offset))
        if ("parameters" in operation or not isinstance(inputs, list) or len(inputs) > 64 or
                any(not valid_step(step) for step in inputs)):
            raise ValueError("SplitEdge requires bounded finite point/distance inputs")
    else:
        parameters = operation.get("parameters")
        if (not isinstance(parameters, list) or len(parameters) > 64 or
                any(not finite(t) for t in parameters)):
            raise ValueError("SplitEdge requires up to 64 finite parameters")
    if operation.get("finish", "Enter") not in ("Enter", "Cancel"):
        raise ValueError("unsupported SplitEdge finish")
    if operation.get("record_viewport", False) and not any("mouse" in step or "pick" in step for step in operation.get("inputs", [])):
        raise ValueError("viewport calibration requires a point click")
    shared = dict(operation, op="merge_edge_command")
    return merge_edges_probe.validate(shared)


def mouse_command(operation, curve, host):
    validate(operation)
    points = []
    for step in inputs(operation):
        if "pick" in step:
            mode = step["pick"]["osnap"]
            points.append("_Pause" if mode == "Persistent" else "_" + mode + " _Pause")
            continue
        if "distance" in step:
            points.append(format(float(step["distance"]), ".17g"))
            continue
        t = step.get("point", step.get("mouse"))
        if not curve.Domain.T0 <= t <= curve.Domain.T1:
            raise ValueError("SplitEdge parameter outside source edge")
        points.append("_Pause" if "mouse" in step else host["_command_point"](host["_xyz"](curve.PointAt(float(t)))))
    return "_SplitEdge _Pause " + " ".join(points + ["_" + operation.get("finish", "Enter")])


def inputs(operation):
    return operation.get("inputs", [dict(point=t) for t in operation.get("parameters", [])])


def drive(operation, script, curve, host, frames=None):
    """Request later real clicks only after Rhino echoes each corresponding Pause."""
    Rhino, System = host["Rhino"], host["System"]
    view = Rhino.RhinoDoc.ActiveDoc.Views.ActiveView
    viewport = view.ActiveViewport
    picks, aims = [], []
    for step in inputs(operation):
        if "pick" in step:
            point = host["_point"](step["pick"].get("aim", step["pick"]["point"]))
            offset = step["pick"].get("offset", [0, 0])
        elif "mouse" in step:
            point, offset = curve.PointAt(float(step["mouse"])), [0, 0]
        else:
            continue
        pixel = viewport.WorldToClient(point)
        x, y = int(pixel.X) + offset[0], int(pixel.Y) + offset[1]
        if not 1 <= x < viewport.Size.Width - 1 or not 1 <= y < viewport.Size.Height - 1:
            raise ValueError("SplitEdge point pick lies outside the owned viewport")
        screen = view.ClientToScreen(System.Drawing.Point(x, y))
        picks.append((screen.X, screen.Y))
        aims.append((point, offset))
    if not picks:
        return host["_run_surface_script"](script, True)
    import clr
    clr.AddReference("System.Windows.Forms")
    from System.Windows.Forms import Timer
    timer = Timer()
    timer.Interval = 100
    history_before = Rhino.RhinoApp.CommandHistoryWindowText
    sent, errors = [], []
    def tick(sender, event):
        try:
            history = Rhino.RhinoApp.CommandHistoryWindowText
            if not history.startswith(history_before):
                raise ValueError("SplitEdge command history changed during mouse input")
            history = history[len(history_before):]
            index = len(sent)
            if index >= len(picks) or history.count("_Pause") < index + 2:
                return
            click = picks[index]
            if operation.get("record_viewport", False):
                if __package__:
                    from .viewport_capture import capture
                else:
                    from viewport_capture import capture
                # Rhino may adjust the viewport after the command begins.
                # Project and calibrate at the actual prompt, not before RunScript.
                current = view.ActiveViewport
                point, offset = aims[index]
                pixel = current.WorldToClient(point)
                x, y = int(pixel.X)+offset[0], int(pixel.Y)+offset[1]
                if not 1 <= x < current.Size.Width-1 or not 1 <= y < current.Size.Height-1:
                    raise ValueError("calibrated point pick lies outside the owned viewport")
                screen = view.ClientToScreen(System.Drawing.Point(x,y))
                frame = capture(current, point, [x,y], host)
                if frames is not None: frames.append(frame)
                click = (screen.X,screen.Y)
            path = os.path.join(os.path.dirname(os.path.abspath(host["__file__"])), "worker-progress.log")
            with open(path, "a") as stream:
                stream.write("PICK @split:%s:%d %d %d\n" % ((operation["id"], index) + click))
                stream.flush()
            sent.append(index)
        except Exception as error:
            errors.append(str(error))
            timer.Stop()
            # Ask the host to cancel only this worker's owned window, so an
            # invalid calibration does not leave RunScript waiting at Pause.
            try:
                path = os.path.join(os.path.dirname(os.path.abspath(host["__file__"])), "worker-progress.log")
                with open(path, "a") as stream:
                    stream.write("PICK_ABORT %s\n" % operation["id"])
                    stream.flush()
            except Exception as abort_error:
                errors.append("cannot request owned cancellation: " + str(abort_error))
    timer.Tick += tick
    try:
        timer.Start()
        result = host["_run_surface_script"](script, True)
        if errors or len(sent) != len(picks):
            raise ValueError("SplitEdge mouse input incomplete: %s" % errors)
        return result
    finally:
        timer.Stop()
        timer.Tick -= tick
        timer.Dispose()


def run(operation, tolerance, host):
    sources, order = validate(operation)
    frames = []
    def driver(operation, script, curve, host):
        return drive(operation, script, curve, host, frames)
    with snapping_environment(operation, host):
        value, elapsed = merge_edges_probe.run_owned(operation, tolerance, host, sources, order,
                                                   "SplitEdge", mouse_command, driver)
    if operation.get("record_viewport", False): value["pick_frames"] = frames
    return value, elapsed


@contextmanager
def snapping_environment(operation, host):
    if not any("pick" in step for step in inputs(operation)):
        yield
        return
    settings = host["Rhino"].ApplicationSettings
    aid, track = settings.ModelAidSettings, settings.SmartTrackSettings
    original_aid, original_track = aid.GetCurrentState(), track.GetCurrentState()
    try:
        aid.GridSnap = aid.Ortho = aid.Planar = False
        aid.Osnap = True
        modes = getattr(settings.OsnapModes, "None")
        names = dict(Point="Point", End="End", Mid="Midpoint", Cen="Center", Quad="Quadrant", Near="Near")
        for name in operation.get("persistent_snaps", []): modes |= getattr(settings.OsnapModes, names[name])
        aid.OsnapModes = modes
        aid.OnlySnapToSelected = False
        aid.OsnapPickboxRadius = 12
        # These probes use Perspective. The separate parallel-view projection
        # property in current online documentation was only added in Rhino 9.
        aid.ProjectSnapToCPlane = False
        aid.SnapToLocked = aid.SnapToOccluded = True
        track.UseSmartTrack = False
        yield
    finally:
        try:
            aid.UpdateFromState(original_aid)
        finally:
            track.UpdateFromState(original_track)
