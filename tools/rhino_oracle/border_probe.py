# -*- coding: utf-8 -*-
"""Owned public-command border probes; imported by the standalone Rhino worker."""


def validate(operation):
    command = operation["command"]
    if command not in ("DupBorder", "DupFaceBorder"):
        raise ValueError("invalid border command")
    layer = operation.get("output_layer", "Current")
    if layer not in ("Current", "Input"):
        raise ValueError("invalid border output layer")
    preselect = operation.get("preselect", False)
    if type(preselect) is not bool:
        raise ValueError("invalid border preselection")
    faces = operation.get("faces")
    if faces is not None and (not faces or len(set(faces)) != len(faces) or
            any(type(i) is not int or i < 0 for i in faces) or not preselect):
        raise ValueError("border face selection needs preselected distinct indices")
    return command, layer, preselect, faces


def run(operation, tolerance, host):
    command, output_layer, preselect, faces = validate(operation)
    if operation["source"]["type"] in ("box", "extrusion", "brep") and not operation.get("artifact_path"):
        raise ValueError("B-rep border probes require compare mode with a shared source artifact")
    Rhino, System = host["Rhino"], host["System"]
    document = Rhino.RhinoDoc.ActiveDoc
    settings = Rhino.DocObjects.ObjectEnumeratorSettings()
    settings.NormalObjects = settings.LockedObjects = settings.HiddenObjects = True
    def objects(): return list(document.Objects.GetObjectList(settings))
    before = set(obj.Id for obj in objects())
    selection_before = [obj.Id for obj in objects() if obj.IsSelected(False)]
    groups_before = set(i for i in range(document.Groups.Count) if not document.Groups.IsDeleted(i))
    layer_before = document.Layers.CurrentLayerIndex
    layers, owned = [], []
    source_id = System.Guid.Empty
    with host["_independent_construction_planes"]() as viewport:
        try:
            viewport.SetConstructionPlane(Rhino.Geometry.Plane.WorldXY)
            document.Objects.UnselectAll()
            for label in ("Source", "Current"):
                layer = Rhino.DocObjects.Layer()
                try:
                    layer.Name = "Viboceros border " + label + " " + str(System.Guid.NewGuid())
                    index = document.Layers.Add(layer)
                    if index < 0: raise ValueError("border layer insertion failed")
                    layers.append(index)
                finally: layer.Dispose()
            if not document.Layers.SetCurrentLayerIndex(layers[1], True):
                raise ValueError("border current layer failed")
            definition = operation["source"]
            if operation.get("artifact_path"):
                path = operation["artifact_path"]
                if path.startswith("/"): path = "Z:" + path.replace("/", "\\")
                model = Rhino.FileIO.File3dm.Read(path)
                if model is None: raise ValueError("cannot read border source artifact")
                try:
                    entries = list(model.Objects)
                    if len(entries) != 1: raise ValueError("border artifact must have one object")
                    geometry = entries[0].Geometry.Duplicate()
                finally: model.Dispose()
            else:
                geometry = host["_object_source"](definition, tolerance)
            if geometry is None: raise ValueError("missing border source geometry")
            owned.append(geometry)
            if not geometry.IsValid: raise ValueError("invalid border source geometry")
            attributes = Rhino.DocObjects.ObjectAttributes()
            try:
                attributes.Name = "Source"
                attributes.LayerIndex = layers[0]
                attributes.ObjectColor = System.Drawing.Color.FromArgb(11, 22, 33)
                attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                if isinstance(geometry, Rhino.Geometry.Surface): source_id = document.Objects.AddSurface(geometry, attributes)
                elif isinstance(geometry, Rhino.Geometry.Brep): source_id = document.Objects.AddBrep(geometry, attributes)
                elif isinstance(geometry, Rhino.Geometry.Mesh): source_id = document.Objects.AddMesh(geometry, attributes)
                else: raise ValueError("unsupported border fixture source")
            finally: attributes.Dispose()
            if source_id == System.Guid.Empty: raise ValueError("border source insertion failed")
            group = document.Groups.Add("Viboceros border source " + str(System.Guid.NewGuid()), [source_id])
            if group < 0: raise ValueError("border source group failed")
            if preselect:
                # Preselected commands skip the option prompt. Set its layer
                # choice using a separate owned surface, never the measured source.
                seed_before = set(obj.Id for obj in objects())
                seed = Rhino.Geometry.PlaneSurface(Rhino.Geometry.Plane.WorldXY, Rhino.Geometry.Interval(0,1), Rhino.Geometry.Interval(0,1))
                try:
                    seed_id = document.Objects.AddSurface(seed)
                    if seed_id == System.Guid.Empty: raise ValueError("border option seed insertion failed")
                    host["_run_surface_script"]("_%s _OutputLayer=_%s _SelID %s _Enter" % (command, output_layer, seed_id), True)
                finally:
                    for obj in objects():
                        if obj.Id not in seed_before: document.Objects.Delete(obj.Id, True)
                    seed.Dispose()
                document.Objects.UnselectAll()
                obj = document.Objects.FindId(source_id)
                if faces is None:
                    document.Objects.Select(source_id)
                else:
                    if not isinstance(obj.Geometry, Rhino.Geometry.Brep) or any(i >= obj.Geometry.Faces.Count for i in faces):
                        raise ValueError("border face index outside source")
                    for index in faces:
                        component = Rhino.Geometry.ComponentIndex(Rhino.Geometry.ComponentIndexType.BrepFace, index)
                        if obj.SelectSubObject(component, True, True, False) == 0:
                            raise ValueError("border face preselection failed")
                script = "_" + command
            else:
                script = "_%s _OutputLayer=_%s _SelID %s _Enter" % (command, output_layer, source_id)
            history_before = Rhino.RhinoApp.CommandHistoryWindowText
            host["_record_progress"]("border command: " + script)
            succeeded = host["_run_surface_script"](script, True)
            created = [obj for obj in objects() if obj.Id not in before and obj.Id != source_id]
            created.sort(key=lambda obj: obj.RuntimeSerialNumber)
            outputs = []
            for obj in created:
                attrs = obj.Attributes
                color = attrs.ObjectColor
                outputs.append(dict(curve=host["_interchange_curve_record"](obj.Geometry),
                    selected=bool(obj.IsSelected(False)), name=attrs.Name or None,
                    layer="Source" if attrs.LayerIndex == layers[0] else "Current" if attrs.LayerIndex == layers[1] else "Unexpected",
                    color=[int(color.R), int(color.G), int(color.B)], color_source=str(attrs.ColorSource),
                    group_count=int(attrs.GroupCount)))
            source = document.Objects.FindId(source_id)
            result = dict(succeeded=bool(succeeded), source_retained=source is not None,
                source_selected=bool(source and source.IsSelected(False)), outputs=outputs,
                new_groups=sum(1 for i in range(document.Groups.Count) if i not in groups_before and i != group and not document.Groups.IsDeleted(i)))
            if operation.get("inspect", False):
                history = Rhino.RhinoApp.CommandHistoryWindowText
                result["history"] = history[len(history_before):] if history.startswith(history_before) else history[-5000:]
                if isinstance(geometry, Rhino.Geometry.Brep):
                    def ends(curve):
                        return [[curve.PointAtStart.X,curve.PointAtStart.Y,curve.PointAtStart.Z],
                                [curve.PointAtEnd.X,curve.PointAtEnd.Y,curve.PointAtEnd.Z]]
                    result["source_edges"] = [ends(e) for e in geometry.Edges]
                    result["source_faces"] = [dict(reversed=f.OrientationIsReversed,
                        loops=[[(t.Edge.EdgeIndex if t.Edge else None, t.IsReversed(), str(t.TrimType)) for t in loop.Trims] for loop in f.Loops]) for f in geometry.Faces]
                    naked = geometry.DuplicateNakedEdgeCurves(True, False)
                    try: result["naked_api"] = [dict(ends=ends(c), domain=[c.Domain.T0,c.Domain.T1]) for c in naked]
                    finally:
                        for c in naked: c.Dispose()
            return result, 0
        finally:
            Rhino.RhinoApp.RunScript("!", False)
            for obj in objects():
                if obj.Id not in before: document.Objects.Delete(obj.Id, True)
            for i in range(document.Groups.Count):
                if i not in groups_before and not document.Groups.IsDeleted(i): document.Groups.Delete(i)
            document.Layers.SetCurrentLayerIndex(layer_before, True)
            for i in reversed(layers): document.Layers.Delete(i, True)
            document.Objects.UnselectAll()
            for key in selection_before: document.Objects.Select(key)
            for geometry in reversed(owned): geometry.Dispose()
