# -*- coding: utf-8 -*-
"""Run a copy of this script in an empty folder inside licensed Rhino 8.

Creates a small public-API 3DM reference containing nested analytic segments.
Use an isolated oracle instance: this script exits Rhino when finished.
Existing reference files are never overwritten. No document objects are changed.
"""

import json
import math
import os

import Rhino
import System


def generate():
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "rhino8_nested_polycurve.3dm")
    if os.path.exists(path):
        raise ValueError("reference already exists")
    point = Rhino.Geometry.Point3d
    first = Rhino.Geometry.LineCurve(point(-2, 0, 0), point(0, 0, 0))
    # Quarter circle centered at (0, 1, 0), from (0, 0, 0) to (1, 1, 0).
    arc = Rhino.Geometry.ArcCurve(Rhino.Geometry.Arc(point(0, 0, 0), point(math.sqrt(0.5), 1 - math.sqrt(0.5), 0), point(1, 1, 0)))
    last = Rhino.Geometry.LineCurve(point(1, 1, 0), point(1, 3, 0))
    inner = Rhino.Geometry.PolyCurve()
    outer = Rhino.Geometry.PolyCurve()
    model = Rhino.FileIO.File3dm()
    try:
        if not inner.AppendSegment(first) or not inner.AppendSegment(arc):
            raise ValueError("could not construct inner composite")
        if not outer.AppendSegment(inner) or not outer.AppendSegment(last):
            raise ValueError("could not construct outer composite")
        if not outer.IsNested or not outer.IsValid:
            raise ValueError("reference must be valid and nested")
        outer.Domain = Rhino.Geometry.Interval(-7.0, 13.0)
        layer = model.Layers.AddDefaultLayer("Reference", System.Drawing.Color.FromArgb(12, 34, 56))
        if layer < 0:
            raise ValueError("could not add reference layer")
        attributes = Rhino.DocObjects.ObjectAttributes()
        attributes.LayerIndex = layer
        attributes.Name = "nested line-arc-line"
        if model.Objects.AddCurve(outer, attributes) == System.Guid.Empty or not model.Write(path, 8):
            raise ValueError("could not write reference")
        return {"engine_version": str(Rhino.RhinoApp.Version), "length": outer.GetLength(), "nested": bool(outer.IsNested)}
    finally:
        model.Dispose()
        outer.Dispose()
        inner.Dispose()
        first.Dispose()
        arc.Dispose()
        last.Dispose()


if __name__ == "__main__":
    try:
        result = generate()
    except Exception as error:
        result = {"error": "%s: %s" % (type(error).__name__, error)}
    with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "reference-result.json"), "w") as stream:
        json.dump(result, stream, indent=2)
    Rhino.RhinoApp.Exit(False)
