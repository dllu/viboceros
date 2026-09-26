# -*- coding: utf-8 -*-
"""Rhino Pipe references, run inside the oracle's isolated Xvfb display."""


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
    end_radius = float(operation.get("end_radius", radius))
    local_blending = bool(operation.get("local_blending", True))
    cap_mode = getattr(geometry.PipeCapMode, operation.get("cap", "Round"))
    thickness = operation.get("thickness")
    if radius <= 0.0 or end_radius <= 0.0 or (thickness is not None and float(thickness) <= 0.0):
        raise ValueError("round Pipe radii must be positive")
    pieces = []
    source_id = None
    created_ids = []
    created_types = []
    succeeded = None
    history = None
    try:
        if operation.get("macro"):
            document = Rhino.RhinoDoc.ActiveDoc
            before = set(item.Id for item in document.Objects)
            source_id = document.Objects.AddCurve(rail)
            if source_id == host["System"].Guid.Empty:
                raise ValueError("could not add Pipe rail")
            script = "! " + operation["macro"].replace("{id}", str(source_id))
            succeeded = bool(Rhino.RhinoApp.RunScript(script, True))
            history = Rhino.RhinoApp.CommandHistoryWindowText[-3000:]
            created = [item for item in document.Objects
                       if item.Id not in before and item.Id != source_id]
            created_ids = [item.Id for item in created]
            created_types = [item.Geometry.GetType().Name for item in created]
            pieces = [item.Geometry.Duplicate() if isinstance(item.Geometry, geometry.Brep)
                      else item.Geometry.ToBrep(True)
                      for item in created if isinstance(item.Geometry, (geometry.Brep, geometry.Extrusion))]
        elif thickness is None:
            if end_radius == radius:
                pieces = list(geometry.Brep.CreatePipe(
                    rail, radius, local_blending, cap_mode, False,
                    tolerance["absolute"], tolerance["angular"]))
            else:
                numbers = host["System"].Array[host["System"].Double]
                pieces = list(geometry.Brep.CreatePipe(
                    rail, numbers([0.0, 1.0]), numbers([radius, end_radius]),
                    local_blending, cap_mode, False,
                    tolerance["absolute"], tolerance["angular"]))
        else:
            if end_radius == radius:
                pieces = list(geometry.Brep.CreateThickPipe(
                    rail, radius, radius + float(thickness), local_blending,
                    cap_mode, False,
                    tolerance["absolute"], tolerance["angular"]))
            else:
                numbers = host["System"].Array[host["System"].Double]
                pieces = list(geometry.Brep.CreateThickPipe(
                    rail, numbers([0.0, 1.0]),
                    numbers([radius, end_radius]),
                    numbers([radius + float(thickness), end_radius + float(thickness)]),
                    local_blending, cap_mode, False,
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
        value = {"pieces": records}
        if succeeded is not None:
            value.update({"succeeded": succeeded, "history": history,
                          "created": len(created_ids), "created_types": created_types})
        return value, 0
    finally:
        for piece in pieces:
            piece.Dispose()
        if source_id is not None:
            Rhino.RhinoApp.RunScript("!", False)
            for item_id in created_ids + [source_id]:
                Rhino.RhinoDoc.ActiveDoc.Objects.Delete(item_id, True)
        rail.Dispose()
