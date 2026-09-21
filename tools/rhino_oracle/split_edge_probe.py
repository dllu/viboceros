# -*- coding: utf-8 -*-
"""Actual SplitEdge command, with a mouse-selected component and bounded point input."""
import math
if __package__:
    from . import merge_edges_probe
else:
    import merge_edges_probe


def validate(operation):
    if (operation.get("op") != "split_edge_command" or operation.get("pick") != "mouse" or
            any(key in operation for key in ("choice", "cancel", "preselect"))):
        raise ValueError("SplitEdge requires mouse component selection")
    parameters = operation.get("parameters")
    if (not isinstance(parameters, list) or len(parameters) > 64 or
            any(type(t) not in (int, float) or math.isnan(t) or math.isinf(t) for t in parameters)):
        raise ValueError("SplitEdge requires up to 64 finite parameters")
    if operation.get("finish", "Enter") not in ("Enter", "Cancel"):
        raise ValueError("unsupported SplitEdge finish")
    shared = dict(operation, op="merge_edge_command")
    return merge_edges_probe.validate(shared)


def mouse_command(operation, curve, host):
    validate(operation)
    points = []
    for t in operation["parameters"]:
        if not curve.Domain.T0 <= t <= curve.Domain.T1:
            raise ValueError("SplitEdge parameter outside source edge")
        points.append(host["_command_point"](host["_xyz"](curve.PointAt(float(t)))))
    return "_SplitEdge _Pause " + " ".join(points + ["_" + operation.get("finish", "Enter")])


def run(operation, tolerance, host):
    sources, order = validate(operation)
    return merge_edges_probe.run_owned(operation, tolerance, host, sources, order,
                                      "SplitEdge", mouse_command)
