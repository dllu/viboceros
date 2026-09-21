# -*- coding: utf-8 -*-
"""Public JoinBreps API on owned, identical shared 3dm sources.

This is a geometry API probe, not evidence for interactive Join behavior.
No objects are inserted in a Rhino document.
"""
import math


def validate(operation):
    paths = operation.get("artifact_paths")
    if not isinstance(paths, list) or not paths or any(
            not isinstance(p, (str, type(u""))) or not p for p in paths):
        raise ValueError("B-rep Join requires shared source artifacts")
    distance = operation.get("join_tolerance")
    if isinstance(distance, bool) or not isinstance(distance, (int, float)) or \
            math.isnan(distance) or math.isinf(distance) or distance < 0:
        raise ValueError("B-rep Join requires a finite nonnegative distance")


def geometry_record(brep, tolerance, host):
    import cap_probe
    record = cap_probe.geometry_record(brep, tolerance, host)
    record["vertex_tolerances"] = [float(v.Tolerance) for v in brep.Vertices]
    record["edge_tolerances"] = [float(e.Tolerance) for e in brep.Edges]
    record["surfaces"] = [host["_nurbs_surface_definition"](f.UnderlyingSurface())
        for f in brep.Faces]
    record["face_reversed"] = [bool(f.OrientationIsReversed) for f in brep.Faces]
    record["trim_curves"] = [[[host["_nurbs_parameter_curve_definition"](t)
        for t in loop.Trims] for loop in f.Loops] for f in brep.Faces]
    return record


def run(operation, tolerance, host):
    validate(operation)
    Rhino = host["Rhino"]
    owned, outputs = [], []
    try:
        for path in operation["artifact_paths"]:
            if path.startswith("/"): path = "Z:" + path.replace("/", "\\")
            model = Rhino.FileIO.File3dm.Read(path)
            if model is None: raise ValueError("cannot read B-rep Join artifact")
            try:
                entries = list(model.Objects)
                if len(entries) != 1: raise ValueError("B-rep Join artifact must have one object")
                geometry = entries[0].Geometry.Duplicate()
            finally: model.Dispose()
            if geometry is None: raise ValueError("missing B-rep Join source")
            owned.append(geometry)
            if not isinstance(geometry, Rhino.Geometry.Brep) or not geometry.IsValid:
                raise ValueError("invalid B-rep Join source")
        before = [geometry_record(g, tolerance, host) for g in owned]
        result = Rhino.Geometry.Brep.JoinBreps(owned, float(operation["join_tolerance"]))
        if result is not None: outputs = list(result)
        if any(not g.IsValid for g in outputs): raise ValueError("invalid B-rep Join output")
        if [geometry_record(g, tolerance, host) for g in owned] != before:
            raise ValueError("B-rep Join mutated inputs")
        return dict(inputs=before, outputs=[geometry_record(g, tolerance, host)
            for g in outputs]), 0
    finally:
        for geometry in outputs + owned: geometry.Dispose()
