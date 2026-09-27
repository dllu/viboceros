# -*- coding: utf-8 -*-
"""Observe Rhino's PolygonCount report for explicit polygon meshes."""


def run(operation, host):
    Rhino = host["Rhino"]
    document = Rhino.RhinoDoc.ActiveDoc
    mesh = host["_polygon_mesh"](operation["vertices"], operation["faces"])
    object_id = None
    try:
        if operation.get("ngon"):
            boundary, faces = operation["ngon"]
            numbers = host["System"].Array[host["System"].Int32]
            ngon = Rhino.Geometry.MeshNgon.Create(numbers(boundary), numbers(faces))
            if ngon is None or mesh.Ngons.AddNgon(ngon) < 0:
                raise ValueError("could not add PolygonCount n-gon")
        object_id = document.Objects.AddMesh(mesh)
        if object_id == host["System"].Guid.Empty:
            raise ValueError("could not add PolygonCount mesh")
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
                "face_count": int(mesh.Faces.Count),
                "ngons": int(mesh.Ngons.Count),
                "triangles": int(mesh.Faces.TriangleCount),
                "quads": int(mesh.Faces.QuadCount)}, 0
    finally:
        Rhino.RhinoApp.RunScript("!", False)
        if object_id is not None:
            document.Objects.Delete(object_id, True)
        mesh.Dispose()
