# -*- coding: utf-8 -*-
"""Owned, public-command mesh joining; no topology or order normalization."""


def validate(operation):
    import math
    sources = operation["sources"]
    if not isinstance(sources, list) or not 1 <= len(sources) <= 32:
        raise ValueError("mesh join requires 1 to 32 sources")
    order = operation.get("selected", list(range(len(sources))))
    if (not isinstance(order, list) or not order or
            any(type(i) is not int or i < 0 or i >= len(sources) for i in order) or
            len(set(order)) != len(order)):
        raise ValueError("invalid mesh join selection")
    for key in ("join_disjoint", "preselect"):
        if type(operation.get(key, False)) is not bool:
            raise ValueError("mesh join options must be boolean")
    absolute = operation.get("absolute_tolerance")
    if absolute is not None and (type(absolute) not in (int, float) or math.isnan(absolute) or math.isinf(absolute) or absolute <= 0):
        raise ValueError("invalid mesh join tolerance")
    return sources, order


def run(operation, tolerance, host):
    sources, order = validate(operation)
    Rhino, System = host["Rhino"], host["System"]
    document = Rhino.RhinoDoc.ActiveDoc
    settings = Rhino.DocObjects.ObjectEnumeratorSettings()
    settings.NormalObjects = settings.LockedObjects = settings.HiddenObjects = True
    def objects(): return list(document.Objects.GetObjectList(settings))
    before = set(obj.Id for obj in objects())
    selected_before = [obj.Id for obj in objects() if obj.IsSelected(False)]
    layer_before = document.Layers.CurrentLayerIndex
    tolerance_before = document.ModelAbsoluteTolerance
    layers, groups, ids = [], [], []
    def source_mesh(source):
        mesh = Rhino.Geometry.Mesh()
        try:
            mesh.Vertices.UseDoublePrecisionVertices = True
            for point in source["vertices"]:
                if mesh.Vertices.Add(host["_point"](point)) < 0:
                    raise ValueError("mesh join double-precision source assignment failed")
            for face in source["faces"]:
                if len(face) not in (3, 4) or any(type(i) is not int or i < 0 for i in face):
                    raise ValueError("invalid mesh join face")
                if mesh.Faces.AddFace(*face) < 0:
                    raise ValueError("mesh join face insertion failed")
            if not mesh.IsValid:
                raise ValueError("invalid mesh join source geometry")
            if host["_polygon_mesh_value"](mesh) != source:
                raise ValueError("mesh join source changed in Rhino construction")
            return mesh
        except Exception:
            mesh.Dispose()
            raise
    try:
        if operation.get("absolute_tolerance") is not None:
            document.ModelAbsoluteTolerance = operation["absolute_tolerance"]
        document.Objects.UnselectAll()
        for index, source in enumerate(sources):
            layer = Rhino.DocObjects.Layer()
            attributes = Rhino.DocObjects.ObjectAttributes()
            mesh = None
            try:
                layer.Name = "Viboceros mesh join " + str(System.Guid.NewGuid())
                layer_index = document.Layers.Add(layer)
                if layer_index < 0: raise ValueError("mesh join layer creation failed")
                layers.append(layer_index)
                attributes.LayerIndex = layer_index
                attributes.Name = "source-%d" % index
                attributes.ObjectColor = System.Drawing.Color.FromArgb(10 + index, 30, 50)
                attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                mesh = source_mesh(source)
                object_id = document.Objects.AddMesh(mesh, attributes)
                if object_id == System.Guid.Empty: raise ValueError("mesh join insertion failed")
                ids.append(object_id)
                if host["_polygon_mesh_value"](document.Objects.FindId(object_id).Geometry) != source:
                    raise ValueError("mesh join source changed during document insertion")
            finally:
                if mesh is not None: mesh.Dispose()
                attributes.Dispose()
                layer.Dispose()
        for members in [[object_id] for object_id in ids] + [ids]:
            index = document.Groups.Add("Viboceros mesh join " + str(System.Guid.NewGuid()), members)
            if index < 0: raise ValueError("mesh join group creation failed")
            groups.append(index)
        option = "_JoinDisjointMeshes=" + ("Yes" if operation.get("join_disjoint", False) else "No")
        selectors = " ".join("_SelID %s" % ids[i] for i in order)
        if operation.get("preselect", False):
            # Preselection executes immediately. Seed its remembered option
            # using separate owned objects, never the measured sources.
            seed_before = set(obj.Id for obj in objects())
            seed = source_mesh(sources[0])
            try:
                seed_ids = [document.Objects.AddMesh(seed) for _ in range(2)]
                if any(i == System.Guid.Empty for i in seed_ids):
                    raise ValueError("mesh join option seed failed")
                seed_selectors = " ".join("_SelID %s" % i for i in seed_ids)
                host["_run_surface_script"]("_-Join %s %s _Enter" % (option, seed_selectors), True)
            finally:
                for obj in objects():
                    if obj.Id not in seed_before: document.Objects.Delete(obj.Id, True)
                seed.Dispose()
            document.Objects.UnselectAll()
            for i in order: document.Objects.Select(ids[i])
            script = "_-Join _Enter"
        else:
            script = "_-Join %s %s _Enter" % (option, selectors)
        host["_record_progress"]("mesh join command: " + script)
        succeeded = host["_run_surface_script"](script, True)
        created = [obj for obj in objects() if obj.Id not in before]
        created.sort(key=lambda obj: obj.RuntimeSerialNumber)
        records = []
        for obj in created:
            if not isinstance(obj.Geometry, Rhino.Geometry.Mesh):
                raise ValueError("mesh join returned non-mesh geometry")
            attributes = obj.Attributes
            color = attributes.ObjectColor
            records.append(dict(mesh=host["_polygon_mesh_value"](obj.Geometry),
                source=ids.index(obj.Id) if obj.Id in ids else None,
                selected=bool(obj.IsSelected(False)), name=attributes.Name,
                layer=layers.index(attributes.LayerIndex) if attributes.LayerIndex in layers else None,
                color=[int(color.R), int(color.G), int(color.B)],
                groups=sorted(groups.index(g) for g in (attributes.GetGroupList() or []) if g in groups)))
        result = dict(succeeded=bool(succeeded), objects=records)
        if operation.get("absolute_tolerance") is not None:
            result["absolute_tolerance"] = float(document.ModelAbsoluteTolerance)
        return result, 0
    finally:
        document.ModelAbsoluteTolerance = tolerance_before
        document.Objects.UnselectAll()
        for obj in objects():
            if obj.Id not in before: document.Objects.Delete(obj.Id, True)
        for index in groups: document.Groups.Delete(index)
        document.Layers.SetCurrentLayerIndex(layer_before, True)
        for index in reversed(layers): document.Layers.Delete(index, True)
        for object_id in selected_before: document.Objects.Select(object_id)
