# -*- coding: utf-8 -*-
"""Observe Rhino's public ExtractMeshFacesByDraftAngle command on owned meshes."""

def run(operation, tolerance, host):
    Rhino, System = host["Rhino"], host["System"]
    vertices = operation.get("vertices")
    faces = operation.get("faces")
    if not isinstance(vertices, list) or not isinstance(faces, list):
        raise ValueError("draft-angle probe requires mesh vertices and faces")
    if not vertices or not faces or any(len(face) not in (3, 4) for face in faces):
        raise ValueError("draft-angle probe requires triangle or quad faces")
    view = operation.get("view", "Top")
    if view not in ("Top", "Bottom"):
        raise ValueError("draft-angle probe accepts Top or Bottom view")
    mode = operation.get("mode", "defaults")
    if mode not in ("defaults", "zero", "right_angle"):
        raise ValueError("unsupported draft-angle probe mode")
    document = Rhino.RhinoDoc.ActiveDoc
    settings = Rhino.DocObjects.ObjectEnumeratorSettings()
    settings.NormalObjects = settings.LockedObjects = settings.HiddenObjects = True
    def objects():
        return list(document.Objects.GetObjectList(settings))
    before_ids = set(obj.Id for obj in objects())
    selected_before = [obj.Id for obj in objects() if obj.IsSelected(False)]
    mesh = host["_polygon_mesh"](vertices, faces)
    attributes = Rhino.DocObjects.ObjectAttributes()
    command_history = Rhino.RhinoApp.CommandHistoryWindowText
    try:
        document.Objects.UnselectAll()
        attributes.Name = "Draft angle probe source"
        source_id = document.Objects.AddMesh(mesh, attributes)
        if source_id == System.Guid.Empty:
            raise ValueError("could not add draft-angle source")
        if not Rhino.RhinoApp.RunScript("_SetView _World _" + view, False):
            raise ValueError("could not set draft-angle view")
        document.Objects.Select(source_id)
        if mode == "defaults":
            macro = "_-ExtractMeshFacesByDraftAngle _Enter _Enter"
        else:
            angle = 0 if mode == "zero" else 90
            macro = ("_-ExtractMeshFacesByDraftAngle "
                "_StartAngleFromCameraDir=%d _EndAngleFromCameraDir=%d _Enter" % (angle, angle))
        succeeded = bool(Rhino.RhinoApp.RunScript(macro, True))
        history_after = Rhino.RhinoApp.CommandHistoryWindowText
        history = history_after[len(command_history):] if history_after.startswith(command_history) else history_after
        records = []
        for obj in objects():
            if obj.Id in before_ids:
                continue
            geometry = obj.Geometry
            if not isinstance(geometry, Rhino.Geometry.Mesh):
                raise ValueError("draft-angle command produced non-mesh geometry")
            if not geometry.FaceNormals.ComputeFaceNormals():
                raise ValueError("could not compute draft-angle face normals")
            records.append(dict(source=obj.Id == source_id,
                selected=bool(obj.IsSelected(False)),
                faces=int(geometry.Faces.Count),
                normal_z=[float(geometry.FaceNormals[index].Z) for index in range(geometry.Faces.Count)]))
        return dict(succeeded=succeeded, view=view, mode=mode, objects=records,
                    history=history[-3000:]), 0
    finally:
        document.Objects.UnselectAll()
        for obj in objects():
            if obj.Id not in before_ids:
                document.Objects.Delete(obj.Id, True)
        mesh.Dispose()
        attributes.Dispose()
        for object_id in selected_before:
            document.Objects.Select(object_id)
