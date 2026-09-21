# -*- coding: utf-8 -*-
"""Owned public Cap command on an identical shared 3dm source.

Faces are identified by oriented spatial-edge incidence. Generated UV frames,
loop start positions and cap-face insertion order are deliberately not compared.
"""


def validate(operation):
    path = operation.get("artifact_path")
    if not isinstance(path, (str, type(u""))) or not path:
        raise ValueError("Cap probes require compare mode with a shared source artifact")
    for key in ("preselect", "reversed"):
        if type(operation.get(key, False)) is not bool:
            raise ValueError("invalid Cap " + key)
    faces = operation.get("keep_faces")
    if faces is not None and (not isinstance(faces, list) or not faces or
            any(type(i) is not int or i < 0 for i in faces) or len(set(faces)) != len(faces)):
        raise ValueError("Cap face subset requires distinct nonnegative indices")
    order = operation.get("edge_order")
    if order is not None and (not isinstance(order, list) or
            any(type(i) is not int or i < 0 for i in order) or sorted(order) != list(range(len(order)))):
        raise ValueError("Cap edge order must be a permutation")


def geometry_record(brep, tolerance, host):
    Rhino = host["Rhino"]
    absolute, relative = min(tolerance["absolute"], 1e-12), min(tolerance["relative"], 1e-13)
    faces = []
    for face in brep.Faces:
        single = face.DuplicateFace(False)
        try:
            mass = Rhino.Geometry.AreaMassProperties.Compute(single, True, False, False, False,
                relative, absolute)
            if mass is None: raise ValueError("Cap face area failed")
            try:
                loops = []
                for loop in face.Loops:
                    edges = [[t.Edge.EdgeIndex if t.Edge else None,
                        bool(t.IsReversed()) ^ bool(face.OrientationIsReversed)] for t in loop.Trims]
                    edges.sort(key=lambda e: (-1 if e[0] is None else e[0], e[1]))
                    loops.append([str(loop.LoopType), edges])
                loops.sort()
                faces.append([loops, float(mass.Area)])
            finally: mass.Dispose()
        finally: single.Dispose()
    faces.sort(key=lambda f: f[0])
    volume = None
    if brep.IsSolid:
        mass = Rhino.Geometry.VolumeMassProperties.Compute(brep, True, False, False, False,
            relative, absolute)
        if mass is None: raise ValueError("Cap volume failed")
        try: volume = float(mass.Volume)
        finally: mass.Dispose()
    edges = []
    for edge in brep.Edges:
        curve = edge.ToNurbsCurve()
        try:
            edges.append(dict(vertices=[edge.StartVertex.VertexIndex, edge.EndVertex.VertexIndex],
                uses=len(edge.TrimIndices()), curve=host["_interchange_curve_record"](curve)))
        finally: curve.Dispose()
    return dict(vertices=[host["_xyz"](v.Location) for v in brep.Vertices], edges=edges,
        faces=faces, solid=bool(brep.IsSolid), manifold=bool(brep.IsManifold), volume=volume)


def run(operation, tolerance, host):
    validate(operation)
    import join_probe
    Rhino, System = host["Rhino"], host["System"]
    document = Rhino.RhinoDoc.ActiveDoc
    settings = Rhino.DocObjects.ObjectEnumeratorSettings()
    settings.NormalObjects = settings.LockedObjects = settings.HiddenObjects = True
    def objects(): return list(document.Objects.GetObjectList(settings))
    before = set(obj.Id for obj in objects())
    selected_before = [obj.Id for obj in objects() if obj.IsSelected(False)]
    groups_before = set(i for i in range(document.Groups.Count) if not document.Groups.IsDeleted(i))
    layer_before = document.Layers.CurrentLayerIndex
    owned, layers = [], []
    try:
        document.Objects.UnselectAll()
        for label in ("Source", "Current"):
            layer = Rhino.DocObjects.Layer()
            try:
                layer.Name = "Viboceros Cap " + label + " " + str(System.Guid.NewGuid())
                index = document.Layers.Add(layer)
                if index < 0: raise ValueError("Cap layer insertion failed")
                layers.append(index)
            finally: layer.Dispose()
        if not document.Layers.SetCurrentLayerIndex(layers[1], True):
            raise ValueError("Cap current layer failed")
        path = operation["artifact_path"]
        if path.startswith("/"): path = "Z:" + path.replace("/", "\\")
        model = Rhino.FileIO.File3dm.Read(path)
        if model is None: raise ValueError("cannot read Cap source artifact")
        try:
            entries = list(model.Objects)
            if len(entries) != 1: raise ValueError("Cap artifact must have one object")
            geometry = entries[0].Geometry.Duplicate()
        finally: model.Dispose()
        if geometry is None: raise ValueError("missing Cap source")
        owned.append(geometry)
        if not isinstance(geometry, Rhino.Geometry.Brep) or not geometry.IsValid:
            raise ValueError("invalid Cap source B-rep")
        before_geometry = geometry_record(geometry, tolerance, host)
        attrs = Rhino.DocObjects.ObjectAttributes()
        try:
            attrs.Name = "Source"
            attrs.LayerIndex = layers[0]
            attrs.ObjectColor = System.Drawing.Color.FromArgb(11,22,33)
            attrs.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
            # Keep the imported topology intact; insertion is not part of Cap.
            source = document.Objects.AddBrep(geometry, attrs, None, False, False)
        finally: attrs.Dispose()
        if source == System.Guid.Empty: raise ValueError("Cap source insertion failed")
        if geometry_record(document.Objects.FindId(source).Geometry, tolerance, host) != before_geometry:
            raise ValueError("Cap insertion changed source geometry")
        group = document.Groups.Add("Viboceros Cap source " + str(System.Guid.NewGuid()), [source])
        if group < 0: raise ValueError("Cap group insertion failed")
        if operation.get("preselect", False):
            document.Objects.Select(source)
            script = "_Cap"
        else:
            script = "_Cap _SelID %s _Enter" % source
        def snapshot():
            result = []
            for obj in sorted([o for o in objects() if o.Id not in before], key=lambda o:o.RuntimeSerialNumber):
                attrs, color = obj.Attributes, obj.Attributes.ObjectColor
                result.append(dict(source=obj.Id == source, name=attrs.Name or None,
                    selected=bool(obj.IsSelected(False)), layer="Source" if attrs.LayerIndex == layers[0] else "Unexpected",
                    color=[int(color.R),int(color.G),int(color.B)], color_source=str(attrs.ColorSource),
                    group_count=int(attrs.GroupCount), geometry=geometry_record(obj.Geometry, tolerance, host)))
            return result
        succeeded, records, unused = join_probe.observe_command(Rhino.Commands.Command, "Cap",
            lambda: host["_run_surface_script"](script, True), snapshot, lambda: [])
        return dict(succeeded=succeeded, input=before_geometry, objects=records), 0
    finally:
        document.Objects.UnselectAll()
        for obj in objects():
            if obj.Id not in before: document.Objects.Delete(obj.Id, True)
        for i in range(document.Groups.Count):
            if i not in groups_before and not document.Groups.IsDeleted(i): document.Groups.Delete(i)
        document.Layers.SetCurrentLayerIndex(layer_before, True)
        for i in reversed(layers): document.Layers.Delete(i, True)
        for geometry in owned: geometry.Dispose()
        for object_id in selected_before: document.Objects.Select(object_id)
