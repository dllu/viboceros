# -*- coding: utf-8 -*-
"""Observe Rhino's PolygonCount report for mesh and NURBS objects."""


def run(operation, host):
    Rhino = host["Rhino"]
    document = Rhino.RhinoDoc.ActiveDoc
    geometry = Rhino.Geometry
    kind = operation.get("kind", "mesh")
    mesh = host["_polygon_mesh"](operation["vertices"], operation["faces"]) if kind == "mesh" else None
    source = mesh
    object_id = None
    try:
        if mesh is not None and operation.get("ngon"):
            boundary, faces = operation["ngon"]
            numbers = host["System"].Array[host["System"].Int32]
            ngon = geometry.MeshNgon.Create(numbers(boundary), numbers(faces))
            if ngon is None or mesh.Ngons.AddNgon(ngon) < 0:
                raise ValueError("could not add PolygonCount n-gon")
        if kind == "mesh":
            object_id = document.Objects.AddMesh(mesh)
        elif kind == "plane":
            source = geometry.PlaneSurface(
                geometry.Plane.WorldXY,
                geometry.Interval(0.0, 2.0), geometry.Interval(0.0, 3.0))
            object_id = document.Objects.AddSurface(source)
        elif kind == "sphere":
            source = geometry.Sphere(geometry.Point3d(0.0, 0.0, 0.0), 1.0).ToBrep()
            object_id = document.Objects.AddBrep(source)
        elif kind == "box":
            box = geometry.Box(geometry.Plane.WorldXY,
                               geometry.Interval(0.0, 2.0),
                               geometry.Interval(0.0, 3.0),
                               geometry.Interval(0.0, 4.0))
            source = box.ToBrep()
            object_id = document.Objects.AddBrep(source)
        else:
            raise ValueError("unknown PolygonCount source kind")
        if object_id == host["System"].Guid.Empty:
            raise ValueError("could not add PolygonCount source")
        before = Rhino.RhinoApp.CommandHistoryWindowText
        script = "! _PolygonCount _SelID {} _Enter".format(object_id)
        succeeded = bool(Rhino.RhinoApp.RunScript(script, True))
        history = Rhino.RhinoApp.CommandHistoryWindowText
        if history.startswith(before):
            history = history[len(before):]
        report = [line.strip() for line in history.splitlines()
                  if line.startswith("There are ") or line.startswith("There would be ")]
        if not succeeded or len(report) != 2:
            raise ValueError("Rhino PolygonCount did not return its two count lines")
        return {"report": "\n".join(report),
                "face_count": int(mesh.Faces.Count) if mesh is not None else None,
                "ngons": int(mesh.Ngons.Count) if mesh is not None else None,
                "triangles": int(mesh.Faces.TriangleCount) if mesh is not None else None,
                "quads": int(mesh.Faces.QuadCount) if mesh is not None else None}, 0
    finally:
        Rhino.RhinoApp.RunScript("!", False)
        if object_id is not None:
            document.Objects.Delete(object_id, True)
        if source is not None:
            source.Dispose()
