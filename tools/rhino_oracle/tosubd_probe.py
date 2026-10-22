"""Observe ToSubD through public command and geometry APIs in an owned document."""
from copy import deepcopy
try:
    from .tosubd_cases import DISCOVERY, all_cases, validate
except (ImportError, ValueError):
    from tosubd_cases import DISCOVERY, all_cases, validate


def run(operation, host, iterations, tolerance):
    validate(operation)
    from join_probe import observe_command
    from subd_box_probe import subd_value
    R, S = host['Rhino'], host['System']
    G = R.Geometry
    doc = R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or doc.UndoRecordingIsActive:
        raise ValueError('empty owned idle document required')
    case = deepcopy(all_cases()[operation['case']])
    ids, layers = [], []
    sdk_inputs = []
    source_files = []
    group = -1
    old_layer = doc.Layers.CurrentLayerIndex
    old_tolerance = doc.ModelAbsoluteTolerance
    if 'absolute_tolerance' in case:
        doc.ModelAbsoluteTolerance = case['absolute_tolerance']

    def geometry(source):
        kind = source['kind']
        if kind == 'mesh':
            result = G.Mesh()
            result.Vertices.UseDoublePrecisionVertices = True
            for point in source['vertices']:
                result.Vertices.Add(G.Point3d(*point))
            for face in source['faces']:
                result.Faces.AddFace(*face)
            for ngon in source.get('ngons', []):
                result.Ngons.AddNgon(G.MeshNgon.Create(S.Array[S.UInt32](ngon['vertices']), S.Array[S.UInt32](ngon['faces'])))
            result.Normals.ComputeNormals()
            return result
        if kind == 'surface':
            return host['_nurbs_surface_from_definition'](source)
        if kind == 'hemisphere_surface':
            sphere=G.Sphere(G.Point3d.Origin,2.).ToNurbsSurface()
            try:
                domain=sphere.Domain(1)
                return sphere.Trim(sphere.Domain(0),G.Interval(domain.T0,(domain.T0+domain.T1)*.5))
            finally:
                sphere.Dispose()
        if kind == 'cone_surface':
            brep=G.Cone(G.Plane.WorldXY,3.,2.).ToBrep(False)
            try:
                nurbs=brep.Faces[0].ToNurbsSurface()
                if 'degree_v' in source and not nurbs.IncreaseDegreeV(source['degree_v']):
                    raise ValueError('cone degree elevation failed')
                return nurbs
            finally:
                brep.Dispose()
        if kind in ['sphere_surface', 'cylinder_surface', 'torus_surface']:
            if kind == 'sphere_surface':
                return G.Sphere(G.Point3d.Origin, 2.).ToNurbsSurface()
            if kind == 'torus_surface':
                return G.Torus(G.Plane.WorldXY, 4., 1.).ToNurbsSurface()
            cylinder = G.Cylinder(G.Circle(G.Plane.WorldXY, 2.), 3.)
            brep = cylinder.ToBrep(False, False)
            try:
                return brep.Faces[0].ToNurbsSurface()
            finally:
                brep.Dispose()
        if kind == 'box':
            return G.Brep.CreateFromBox(G.BoundingBox(G.Point3d(0, 0, 0), G.Point3d(4, 3, 2)))
        if kind == 'extrusion':
            profile = source.get('profile', 'rectangle')
            if profile == 'pipe':
                cylinder = G.Cylinder(G.Circle(G.Plane.WorldXY,2.), source.get('height',2.))
                return G.Extrusion.CreatePipeExtrusion(cylinder,1.,source.get('cap',True),source.get('cap',True))
            if profile == 'rectangle':
                curve = G.Rectangle3d(G.Plane.WorldXY, 4., 3.).ToNurbsCurve()
            elif profile == 'circle':
                curve = G.Circle(G.Plane.WorldXY, 2.).ToNurbsCurve()
            elif profile == 'arc':
                import math
                curve = G.Arc(G.Circle(G.Plane.WorldXY, 2.), math.pi / 2.).ToNurbsCurve()
            elif profile == 'open_kink':
                curve = G.PolylineCurve([G.Point3d(0.,0.,0.), G.Point3d(1.,1.,0.), G.Point3d(3.,0.,0.)])
            else:
                raise ValueError('unknown extrusion profile')
            try:
                extrusion = G.Extrusion.Create(curve, source.get('height', 2.), source.get('cap', True))
            finally:
                curve.Dispose()
            if extrusion is None:
                raise ValueError('extrusion construction failed')
            try:
                if 'inner_radius' in source:
                    inner = G.Circle(G.Plane.WorldXY, source['inner_radius']).ToNurbsCurve()
                    try:
                        if not extrusion.AddInnerProfile(inner):
                            raise ValueError('inner extrusion profile failed')
                    finally:
                        inner.Dispose()
                if 'miter_start' in source:
                    extrusion.MiterPlaneNormalAtStart = G.Vector3d(*source['miter_start'])
                if 'miter_end' in source:
                    extrusion.MiterPlaneNormalAtEnd = G.Vector3d(*source['miter_end'])
                if 'transform' in source:
                    transform = G.Transform.Identity
                    for r in range(4):
                        for c in range(4):
                            transform[r,c] = source['transform'][4*r+c]
                    if not extrusion.Transform(transform):
                        raise ValueError('extrusion transform failed')
                return extrusion
            except Exception:
                extrusion.Dispose()
                raise
        if kind == 'trimmed':
            outer = G.Rectangle3d(G.Plane.WorldXY, 4., 3.).ToNurbsCurve()
            hole = G.Circle(G.Point3d(2., 1.5, 0.), .5).ToNurbsCurve()
            try:
                results = G.Brep.CreatePlanarBreps([outer, hole], tolerance['absolute'])
                if results is None or len(results) != 1:
                    raise ValueError('trimmed source construction failed')
                return results[0]
            finally:
                outer.Dispose()
                hole.Dispose()
        raise ValueError('unknown fixed source')

    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects), key=lambda o: o.RuntimeSerialNumber):
            g, a = obj.Geometry, obj.Attributes
            source = ids.index(obj.Id) if obj.Id in ids else None
            row = dict(role='source' if source is not None else 'output', source=source,
                       selected=bool(obj.IsSelected(False)), name=a.Name,
                       layer=doc.Layers[a.LayerIndex].Name,
                       groups=[doc.Groups.GroupName(i) for i in (a.GetGroupList() or [])],
                       color=[int(a.ObjectColor.R), int(a.ObjectColor.G), int(a.ObjectColor.B)],
                       color_source=int(a.ColorSource), crc=int(g.DataCRC(0)))
            if isinstance(g, G.SubD):
                row['subd'] = subd_value(g, host)
            elif isinstance(g, G.Mesh):
                row['mesh'] = host['_polygon_mesh_value'](g)
                row['selected_faces'] = [i for i in range(g.Faces.Count) if obj.IsSubObjectSelected(G.ComponentIndex(G.ComponentIndexType.MeshFace, i))]
                row['mesh']['ngons'] = [dict(vertices=[int(v) for v in n.BoundaryVertexIndexList()], faces=[int(f) for f in n.FaceIndexList()]) for n in g.Ngons]
            elif isinstance(g, G.Extrusion) or isinstance(g, G.Brep):
                if isinstance(g, G.Extrusion) and case.get('record_extrusion'):
                    row['extrusion'] = dict(profile_count=int(g.ProfileCount), cap_count=int(g.CapCount),
                        path_start=[float(g.PathStart.X), float(g.PathStart.Y), float(g.PathStart.Z)],
                        path_end=[float(g.PathEnd.X), float(g.PathEnd.Y), float(g.PathEnd.Z)],
                        mitered=[bool(g.IsMiteredAtStart),bool(g.IsMiteredAtEnd)])
                brep = g.ToBrep() if isinstance(g, G.Extrusion) else g
                if 'extrusion' in row:
                    row['extrusion']['brep_valid'] = bool(brep.IsValid)
                try:
                    row['selected_faces'] = [i for i in range(brep.Faces.Count) if obj.IsSubObjectSelected(G.ComponentIndex(G.ComponentIndexType.BrepFace, i))]
                    row['brep'] = [dict(surface=host['_nurbs_surface_definition'](face.UnderlyingSurface()), reversed=bool(face.OrientationIsReversed), loops=int(face.Loops.Count)) for face in brep.Faces]
                    if case.get('record_features'):
                        from tosubd_info import surface_info
                        row['features']=[surface_info(face.UnderlyingSurface(),host)for face in brep.Faces]
                finally:
                    if brep is not g:
                        brep.Dispose()
            elif isinstance(g, G.Surface):
                row['surface'] = host['_nurbs_surface_definition'](g)
            else:
                raise ValueError('unexpected conversion geometry')
            rows.append(row)
        return rows

    try:
        record = doc.BeginUndoRecord('Owned ToSubD setup')
        try:
            for name, color in [('Conversion input', S.Drawing.Color.Blue), ('Conversion current', S.Drawing.Color.Green)]:
                layers.append(doc.Layers.Add(name, color))
            group = doc.Groups.Add('Conversion assembly')
            for index, source in enumerate(case['sources']):
                g = geometry(source)
                if g is None or not g.IsValid:
                    raise ValueError('invalid fixed ToSubD source')
                if case.get('sdk'):
                    method=G.SubDFromSurfaceMethods.SubDFriendlyFit if case['options']['UseSurface']=='Location' else G.SubDFromSurfaceMethods.FromNurbsControlNet
                    converted=G.SubD.CreateFromSurface(g,method,case['options']['SurfaceCorners'])
                    try:
                        sdk_inputs.append(dict(source=index,surface=host['_nurbs_surface_definition'](g),subd=None if converted is None else subd_value(converted,host)))
                    finally:
                        if converted is not None:
                            converted.Dispose()
                if case.get('source_3dm'):
                    import os, base64
                    model = R.FileIO.File3dm()
                    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'extrusion-' + str(S.Guid.NewGuid()) + '.3dm')
                    decoded = None
                    try:
                        model.Layers.AddDefaultLayer('Default', S.Drawing.Color.Black)
                        model.Objects.AddExtrusion(g)
                        if not model.Write(path,8):
                            raise ValueError('extrusion source 3DM write failed')
                        decoded = R.FileIO.File3dm.Read(path)
                        if decoded is None or len(list(decoded.Objects)) != 1 or not isinstance(list(decoded.Objects)[0].Geometry,G.Extrusion):
                            raise ValueError('native source 3DM lost extrusion type')
                        with open(path,'rb') as file:
                            payload=base64.b64encode(file.read()).decode('ascii')
                        source_files.append(dict(source=index,base64=payload))
                    finally:
                        if decoded is not None:
                            decoded.Dispose()
                        model.Dispose()
                        if os.path.exists(path):
                            os.unlink(path)
                a = R.DocObjects.ObjectAttributes()
                a.LayerIndex, a.Name = layers[0], 'Input ' + str(index)
                a.ObjectColor = S.Drawing.Color.FromArgb(17, 43, 91)
                a.ColorSource = R.DocObjects.ObjectColorSource.ColorFromObject
                a.AddToGroup(group)
                try:
                    key = doc.Objects.Add(g, a)
                finally:
                    g.Dispose()
                if key == S.Guid.Empty:
                    raise ValueError('cannot add conversion source')
                ids.append(key)
                obj = doc.Objects.FindId(key)
                if source.get('selected_faces'):
                    for face in source['selected_faces']:
                        component = G.ComponentIndex(G.ComponentIndexType.MeshFace if source['kind'] == 'mesh' else G.ComponentIndexType.BrepFace, face)
                        if not obj.SelectSubObject(component, True, True, False):
                            raise ValueError('surface component preselection failed')
                elif case.get('preselected'):
                    doc.Objects.Select(key)
            doc.Layers.SetCurrentLayerIndex(layers[1], True)
        finally:
            doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True)
        before = snapshot()
        tokens = ['_-ToSubD']
        discovery = operation['case'] in DISCOVERY
        def options():
            result = []
            for name, value in case.get('options', {}).items():
                value = ('Yes' if value else 'No') if isinstance(value, bool) else str(value)
                result.append('_' + name + '=_'+value)
            return result
        if discovery:
            tokens.extend(options())
        if case.get('cancel_selection'):
            tokens.append('_Cancel')
        else:
            if not case.get('preselected') and not any(s.get('selected_faces') for s in case['sources']):
                for key in ids:
                    tokens.extend(["'_-SelID", str(key)])
                if not discovery:
                    tokens.append('_Enter')
            if not discovery:
                tokens.extend(options())
            tokens.append('_Cancel' if case.get('cancel') else '_Enter')
        tokens.extend(['_Cancel'] * 3)
        macro = ' '.join(tokens)
        marker = 'Owned ToSubD ' + str(S.Guid.NewGuid())
        R.RhinoApp.WriteLine(marker)
        success, after, events = observe_command(R.Commands.Command, 'ToSubD', lambda: R.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        history = R.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()
        after_script = snapshot()
        R.RhinoApp.RunScript('_Undo', False)
        undo = snapshot()
        R.RhinoApp.RunScript('_Redo', False)
        redo = snapshot()
        return dict(input=case, success=success, macro=macro, history=history, events=events,
                    source_files=source_files, sdk_inputs=sdk_inputs, document_tolerance=float(doc.ModelAbsoluteTolerance), before=before, after=after, after_script=after_script, undo=undo, redo=redo), 0
    finally:
        doc.ModelAbsoluteTolerance = old_tolerance
        doc.Layers.SetCurrentLayerIndex(old_layer, True)
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)
        if group >= 0:
            doc.Groups.Delete(group)
        for layer in reversed(layers):
            doc.Layers.Delete(layer, True)
