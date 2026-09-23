# -*- coding: utf-8 -*-
"""Owned public Rhino Cap probe for polygon meshes."""


def run(operation, tolerance, host):
    Rhino, System = host["Rhino"], host["System"]
    vertices = operation.get("vertices")
    faces = operation.get("faces")
    if not isinstance(vertices, list) or not isinstance(faces, list):
        raise ValueError("mesh Cap requires vertices and faces")
    if not vertices or not faces or any(len(face) not in (3, 4) for face in faces):
        raise ValueError("mesh Cap requires triangle or quad faces")
    document = Rhino.RhinoDoc.ActiveDoc
    settings = Rhino.DocObjects.ObjectEnumeratorSettings()
    settings.NormalObjects = settings.LockedObjects = settings.HiddenObjects = True
    def objects(): return list(document.Objects.GetObjectList(settings))
    before = set(obj.Id for obj in objects())
    selected_before = [obj.Id for obj in objects() if obj.IsSelected(False)]
    mesh = host["_polygon_mesh"](vertices, faces)
    attributes = Rhino.DocObjects.ObjectAttributes()
    try:
        document.Objects.UnselectAll()
        attributes.Name = "Mesh Cap source"
        source = document.Objects.AddMesh(mesh, attributes)
        if source == System.Guid.Empty:
            raise ValueError("could not add mesh Cap source")
        document.Objects.Select(source)
        succeeded = host["_run_surface_script"]("_Cap", True)
        records = []
        for obj in objects():
            if obj.Id in before:
                continue
            geometry = obj.Geometry
            if not isinstance(geometry, Rhino.Geometry.Mesh):
                raise ValueError("mesh Cap produced non-mesh geometry")
            boundary = sum(1 for edge in range(geometry.TopologyEdges.Count)
                if len(geometry.TopologyEdges.GetConnectedFaces(edge)) == 1)
            records.append(dict(source=obj.Id == source, selected=bool(obj.IsSelected(False)),
                name=obj.Attributes.Name, boundary_edges=boundary,
                ngons=int(geometry.Ngons.Count), mesh=host["_polygon_mesh_value"](geometry)))
        return dict(succeeded=bool(succeeded), objects=records), 0
    finally:
        document.Objects.UnselectAll()
        for obj in objects():
            if obj.Id not in before:
                document.Objects.Delete(obj.Id, True)
        mesh.Dispose()
        attributes.Dispose()
        for object_id in selected_before:
            document.Objects.Select(object_id)
