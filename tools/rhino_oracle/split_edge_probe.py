# -*- coding: utf-8 -*-
"""Actual SplitEdge command, with a mouse-selected component and bounded point input."""
import math
import os
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
    if "inputs" in operation:
        inputs = operation["inputs"]
        if ("parameters" in operation or not isinstance(inputs, list) or len(inputs) > 64 or
                any(not isinstance(step, dict) or len(step) != 1 or
                    next(iter(step)) not in ("point", "mouse", "distance") or
                    not finite(next(iter(step.values()))) for step in inputs)):
            raise ValueError("SplitEdge requires bounded finite point/distance inputs")
    else:
        parameters = operation.get("parameters")
        if (not isinstance(parameters, list) or len(parameters) > 64 or
                any(not finite(t) for t in parameters)):
            raise ValueError("SplitEdge requires up to 64 finite parameters")
    if operation.get("finish", "Enter") not in ("Enter", "Cancel"):
        raise ValueError("unsupported SplitEdge finish")
    shared = dict(operation, op="merge_edge_command")
    return merge_edges_probe.validate(shared)


def mouse_command(operation, curve, host):
    validate(operation)
    points = []
    for step in inputs(operation):
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


def drive(operation, script, curve, host):
    """Request later real clicks only after Rhino echoes each corresponding Pause."""
    Rhino, System = host["Rhino"], host["System"]
    view = Rhino.RhinoDoc.ActiveDoc.Views.ActiveView
    viewport = view.ActiveViewport
    picks = []
    for step in inputs(operation):
        if "mouse" not in step:
            continue
        pixel = viewport.WorldToClient(curve.PointAt(float(step["mouse"])))
        if not 1 <= pixel.X < viewport.Size.Width - 1 or not 1 <= pixel.Y < viewport.Size.Height - 1:
            raise ValueError("SplitEdge point pick lies outside the owned viewport")
        screen = view.ClientToScreen(System.Drawing.Point(int(pixel.X), int(pixel.Y)))
        picks.append((screen.X, screen.Y))
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
            path = os.path.join(os.path.dirname(os.path.abspath(host["__file__"])), "worker-progress.log")
            with open(path, "a") as stream:
                stream.write("PICK @split:%s:%d %d %d\n" % ((operation["id"], index) + picks[index]))
                stream.flush()
            sent.append(index)
        except Exception as error:
            errors.append(str(error))
            timer.Stop()
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
    return merge_edges_probe.run_owned(operation, tolerance, host, sources, order,
                                      "SplitEdge", mouse_command, drive)
