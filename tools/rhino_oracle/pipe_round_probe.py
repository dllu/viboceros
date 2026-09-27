# -*- coding: utf-8 -*-
"""Rhino Pipe references, run inside the oracle's isolated Xvfb display."""
import math


def _wall_basis(face, host):
    surface = face.UnderlyingSurface().ToNurbsSurface()
    try:
        points = surface.Points
        return {
            "degree_u": int(surface.Degree(0)),
            "degree_v": int(surface.Degree(1)),
            "knots_u": [float(surface.KnotsU[i]) for i in range(surface.KnotsU.Count)],
            "knots_v": [float(surface.KnotsV[i]) for i in range(surface.KnotsV.Count)],
            "controls": [
                [host["_xyz"](points.GetControlPoint(i, j).Location)
                 for j in range(points.CountV)]
                for i in range(points.CountU)],
            "weights": [
                [float(points.GetControlPoint(i, j).Weight)
                 for j in range(points.CountV)]
                for i in range(points.CountU)],
        }
    finally:
        surface.Dispose()


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
    elif rail_kind == "circle":
        rail = geometry.Circle(geometry.Plane.WorldXY, 5.0).ToNurbsCurve()
    elif rail_kind == "bezier":
        rail = geometry.NurbsCurve(3, False, 4, 4)
        for index, xyz in enumerate([(0, 0, 0), (2, 0, 0),
                                     (3, 2, 1), (5, 0, 2)]):
            rail.Points.SetPoint(index, geometry.Point3d(*xyz))
        for index in range(3):
            rail.Knots[index] = 0.0
            rail.Knots[index + 3] = 1.0
    elif rail_kind == "multispan":
        rail = geometry.NurbsCurve(3, False, 4, 5)
        for index, xyz in enumerate([(0, 0, 0), (1, 1, 0), (2, -1, 1),
                                     (3, 2, 1), (5, 0, 2)]):
            rail.Points.SetPoint(index, geometry.Point3d(*xyz))
        for index in range(3):
            rail.Knots[index] = 0.0
            rail.Knots[index + 4] = 1.0
        rail.Knots[3] = 0.5
    else:
        raise ValueError("invalid round Pipe rail")
    radius = float(operation.get("radius", 1.0))
    end_radius = float(operation.get("end_radius", radius))
    stations = operation.get("stations", [])
    if not isinstance(stations, list) or any(
            not isinstance(station, list) or len(station) != 2
            for station in stations):
        raise ValueError("invalid Pipe radius stations")
    parameters = [0.0] + [float(station[0]) for station in stations] + [1.0]
    radii = [radius] + [float(station[1]) for station in stations] + [end_radius]
    if (any(math.isnan(value) or math.isinf(value) for value in parameters + radii)
            or any(not 0.0 < parameter < 1.0 for parameter in parameters[1:-1])
            or any(a >= b for a, b in zip(parameters, parameters[1:]))
            or any(value <= 0.0 for value in radii)):
        raise ValueError("invalid Pipe radius stations")
    local_blending = bool(operation.get("local_blending", True))
    fit_rail = bool(operation.get("fit_rail", False))
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
            if end_radius == radius and not stations:
                pieces = list(geometry.Brep.CreatePipe(
                    rail, radius, local_blending, cap_mode, fit_rail,
                    tolerance["absolute"], tolerance["angular"]))
            else:
                numbers = host["System"].Array[host["System"].Double]
                pieces = list(geometry.Brep.CreatePipe(
                    rail, numbers(parameters), numbers(radii),
                    local_blending, cap_mode, fit_rail,
                    tolerance["absolute"], tolerance["angular"]))
        else:
            if end_radius == radius and not stations:
                pieces = list(geometry.Brep.CreateThickPipe(
                    rail, radius, radius + float(thickness), local_blending,
                    cap_mode, fit_rail,
                    tolerance["absolute"], tolerance["angular"]))
            else:
                numbers = host["System"].Array[host["System"].Double]
                pieces = list(geometry.Brep.CreateThickPipe(
                    rail, numbers(parameters),
                    numbers(radii),
                    numbers([value + float(thickness) for value in radii]),
                    local_blending, cap_mode, fit_rail,
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
                    "wall_samples": [
                        host["_xyz"](piece.Faces[0].PointAt(
                            piece.Faces[0].Domain(0).ParameterAt(fraction),
                            piece.Faces[0].Domain(1).ParameterAt(0.0)))
                        for fraction in [i / 20.0 for i in range(21)]]
                    if operation.get("stations") else None,
                    "wall_basis": _wall_basis(piece.Faces[0], host)
                    if operation.get("inspect_basis") else None,
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
