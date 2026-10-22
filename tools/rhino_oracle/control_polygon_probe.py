"""Public native control-net and command captures in an owned idle document."""
from copy import deepcopy
try:
    from .control_polygon_cases import SCENES, request, validate
except (ImportError, ValueError):
    from control_polygon_cases import SCENES, request, validate


def run(operation, host, iterations, tolerance):
    validate(operation)
    from join_probe import observe_command
    R, S = host['Rhino'], host['System']
    G, doc = R.Geometry, R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or doc.UndoRecordingIsActive:
        raise ValueError('empty owned idle document required')
    case = deepcopy(SCENES[operation['case']])
    ids, layers = [], []
    old_layer, group = doc.Layers.CurrentLayerIndex, -1

    def geometry(source):
        kind = source['kind']
        if kind in ['subd', 'mesh']:
            mesh = G.Mesh()
            mesh.Vertices.UseDoublePrecisionVertices = True
            for p in source['vertices']:
                mesh.Vertices.Add(G.Point3d(*p))
            for f in source['faces']:
                mesh.Faces.AddFace(*f)
            for ngon in source.get('ngons', []):
                mesh.Ngons.AddNgon(G.MeshNgon.Create(S.Array[S.UInt32](ngon['vertices']), S.Array[S.UInt32](ngon['faces'])))
            if kind == 'mesh':
                return mesh
            try:
                subd = G.SubD.CreateFromMesh(mesh)
            finally:
                mesh.Dispose()
            vertices, vertex = [], subd.Vertices.First
            while vertex is not None:
                vertices.append(vertex)
                vertex = vertex.Next
            indices = {int(v.Id): i for i, v in enumerate(vertices)}
            for edge in subd.Edges:
                pair = sorted([indices[int(edge.VertexFrom.Id)], indices[int(edge.VertexTo.Id)]])
                if pair in source.get('creases', []):
                    edge.Tag = G.SubDEdgeTag.Crease
            subd.UpdateAllTagsAndSectorCoefficients()
            return subd
        if kind == 'line':
            return G.LineCurve(G.Point3d(0., 0., 0.), G.Point3d(4., 3., 2.))
        if kind == 'circle':
            return G.Circle(G.Plane.WorldXY, 3.).ToNurbsCurve()
        if kind == 'polycurve':
            result = G.PolyCurve()
            result.Append(G.Line(G.Point3d(0., 0., 0.), G.Point3d(4., 0., 0.)))
            result.Append(G.Arc(G.Point3d(4., 0., 0.), G.Point3d(5., 1., 0.), G.Point3d(4., 2., 0.)))
            return result
        if kind == 'surface':
            return G.NurbsSurface.CreateFromCorners(*[G.Point3d(*p) for p in [[0.,0.,0.],[4.,0.,1.],[4.,3.,2.],[0.,3.,-1.]]])
        if kind == 'sphere':
            return G.Sphere(G.Point3d(0., 0., 0.), 3.).ToNurbsSurface()
        if kind == 'trimmed':
            sphere = G.Sphere(G.Point3d(0., 0., 0.), 3.).ToNurbsSurface()
            try:
                u, v = sphere.Domain(0), sphere.Domain(1)
                return sphere.Trim(G.Interval(u.T0, (u.T0 + u.T1)*.7), G.Interval(v.T0*.5, v.T1*.5)).ToBrep()
            finally:
                sphere.Dispose()
        if kind == 'box':
            return G.Brep.CreateFromBox(G.BoundingBox(G.Point3d(0.,0.,0.), G.Point3d(4.,3.,2.)))
        if kind == 'extrusion':
            return G.Extrusion.CreateCylinderExtrusion(G.Cylinder(G.Circle(G.Plane.WorldXY,2.),3.), True, True)
        if kind == 'point':
            return G.Point(G.Point3d(1., 2., 3.))
        raise ValueError('unsupported fixed source')

    def mesh_value(mesh):
        result = host['_polygon_mesh_value'](mesh)
        result['ngons'] = [dict(vertices=[int(v) for v in n.BoundaryVertexIndexList()],
                                faces=[int(f) for f in n.FaceIndexList()]) for n in mesh.Ngons]
        result['normals'] = [[float(n.X),float(n.Y),float(n.Z)] for n in mesh.Normals]
        result['texture_coordinates'] = [[float(t.X),float(t.Y)] for t in mesh.TextureCoordinates]
        return result

    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects), key=lambda o: o.RuntimeSerialNumber):
            attrs, g = obj.Attributes, obj.Geometry
            row = dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                       name=attrs.Name, layer=doc.Layers[attrs.LayerIndex].Name,
                       selected=bool(obj.IsSelected(False)), crc=int(g.DataCRC(0)),
                       groups=[doc.Groups.GroupName(i) for i in (attrs.GetGroupList() or [])],
                       color=[int(attrs.ObjectColor.R),int(attrs.ObjectColor.G),int(attrs.ObjectColor.B)],
                       color_source=int(attrs.ColorSource), kind=str(g.ObjectType))
            if isinstance(g, G.Mesh):
                row['mesh'] = mesh_value(g)
            elif isinstance(g, G.Curve):
                ok, polyline = g.TryGetPolyline()
                row['polyline'] = [host['_xyz'](p) for p in polyline] if ok else None
            elif isinstance(g, G.SubD):
                row['subd_faces'] = [int(f.EdgeCount) for f in g.Faces]
            rows.append(row)
        return rows

    def archive():
        model = R.FileIO.File3dm()
        try:
            layer_map = {l.Index:model.Layers.AddLayer(l.Name,l.Color) for l in doc.Layers if not l.IsDeleted}
            for obj in sorted(list(doc.Objects), key=lambda o:o.RuntimeSerialNumber):
                attrs = obj.Attributes.Duplicate()
                attrs.LayerIndex = layer_map[attrs.LayerIndex]
                attrs.RemoveFromAllGroups()
                model.Objects.Add(obj.Geometry, attrs)
            model.Settings.ModelAbsoluteTolerance = doc.ModelAbsoluteTolerance
            model.Settings.ModelUnitSystem = doc.ModelUnitSystem
            return S.Convert.ToBase64String(model.ToByteArray())
        finally:
            model.Dispose()

    try:
        serial = doc.BeginUndoRecord('Owned control polygon setup')
        try:
            for name in ['Control polygon source', 'Control polygon current']:
                layers.append(doc.Layers.Add(name,S.Drawing.Color.FromArgb(40,80,120)))
            group = doc.Groups.Add('Control polygon assembly')
            for index, source in enumerate(case['sources']):
                g = geometry(source)
                try:
                    attrs = R.DocObjects.ObjectAttributes()
                    attrs.Name, attrs.LayerIndex = 'Input '+str(index), layers[0]
                    attrs.ObjectColor = S.Drawing.Color.FromArgb(17,43,91)
                    attrs.ColorSource = R.DocObjects.ObjectColorSource.ColorFromObject
                    attrs.AddToGroup(group)
                    ids.append(doc.Objects.Add(g,attrs))
                finally:
                    g.Dispose()
            doc.Layers.SetCurrentLayerIndex(layers[1],True)
        finally:
            doc.EndUndoRecord(serial)
        doc.ClearUndoRecords(True)
        if case['preselected']:
            for key in ids:
                doc.Objects.Select(key)
        before, source_3dm = snapshot(), archive()
        marker = 'Owned control polygon '+str(S.Guid.NewGuid())
        R.RhinoApp.WriteLine(marker)
        macro = '_-ExtractControlPolygon _OutputLayer=_'+case['layer']
        if 'priority' in case:
            macro += ' _SubDPriority=_'+case['priority']
        if 'tokens' in case:
            macro += ' '+' '.join(case['tokens'])
        if not case['preselected']:
            macro += ' '+ ' '.join('_SelID '+str(key) for key in ids)
        macro += ' _Enter _Cancel _Cancel'
        success, after, events = observe_command(R.Commands.Command, 'ExtractControlPolygon',
            lambda:R.RhinoApp.RunScript(macro,True), snapshot, lambda:[], True)
        history = R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        result_3dm, after_script = archive(), snapshot()
        R.RhinoApp.RunScript('_Undo',False)
        undo = snapshot()
        R.RhinoApp.RunScript('_Redo',False)
        redo = snapshot()
        return dict(input=case, macro=macro, success=success, before=before, after=after,
                    after_script=after_script, undo=undo, redo=redo, events=events,
                    history=history, source_3dm=source_3dm, result_3dm=result_3dm), 0
    finally:
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id,True)
        if group >= 0:
            doc.Groups.Delete(group)
        for layer in reversed(layers):
            doc.Layers.Delete(layer,True)
