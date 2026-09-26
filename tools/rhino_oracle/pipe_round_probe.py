# -*- coding: utf-8 -*-
"""Read-only RhinoCommon round Pipe reference, run inside the oracle's Xvfb."""


def run(operation, tolerance, host):
    Rhino = host["Rhino"]
    geometry = Rhino.Geometry
    rail_kind = operation.get("rail", "line")
    if rail_kind == "line":
        rail = geometry.LineCurve(
            geometry.Point3d(0.0, 0.0, 0.0),
            geometry.Point3d(0.0, 0.0, 5.0))
    elif rail_kind == "arc":
        arc = geometry.Arc(
            geometry.Point3d(5.0, 0.0, 0.0),
            geometry.Point3d(5.0 / 2.0 ** 0.5, 5.0 / 2.0 ** 0.5, 0.0),
            geometry.Point3d(0.0, 5.0, 0.0))
        rail = geometry.ArcCurve(arc)
    else:
        raise ValueError("invalid round Pipe rail")
    radius = float(operation.get("radius", 1.0))
    thickness = operation.get("thickness")
    if radius <= 0.0 or (thickness is not None and float(thickness) <= 0.0):
        raise ValueError("round Pipe radii must be positive")
    pieces = []
    try:
        if thickness is None:
            pieces = list(geometry.Brep.CreatePipe(
                rail, radius, True, geometry.PipeCapMode.Round, False,
                tolerance["absolute"], tolerance["angular"]))
        else:
            pieces = list(geometry.Brep.CreateThickPipe(
                rail, radius, radius + float(thickness), True,
                geometry.PipeCapMode.Round, False,
                tolerance["absolute"], tolerance["angular"]))
        records = []
        for piece in pieces:
            properties = geometry.VolumeMassProperties.Compute(piece)
            if properties is None:
                raise ValueError("Rhino round Pipe volume failed")
            try:
                records.append({
                    "faces": piece.Faces.Count,
                    "closed": bool(piece.IsSolid),
                    "volume": float(properties.Volume),
                    "valid": bool(piece.IsValid),
                    "face_bounds": [
                        [host["_xyz"](face.GetBoundingBox(True).Min),
                         host["_xyz"](face.GetBoundingBox(True).Max)]
                        for face in piece.Faces],
                })
            finally:
                properties.Dispose()
        return {"pieces": records}, 0
    finally:
        for piece in pieces:
            piece.Dispose()
        rail.Dispose()
