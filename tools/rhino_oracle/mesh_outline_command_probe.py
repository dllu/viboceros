"""Public MeshOutline command events and lifecycle in an owned idle scene."""
try:
    from .mesh_outline_command_cases import ALL_CASES, validate
except (ImportError, ValueError):
    from mesh_outline_command_cases import ALL_CASES, validate


def run(operation, host, iterations, tolerance):
    validate(operation)
    from join_probe import observe_command
    R, S = host['Rhino'], host['System']
    G, doc = R.Geometry, R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or doc.UndoRecordingIsActive:
        raise ValueError('owned empty idle document required')
    case = ALL_CASES[operation['case']]
    vp = doc.Views.ActiveView.ActiveViewport
    old_view = R.DocObjects.ViewportInfo(vp)
    old_target, old_plane, old_name = vp.CameraTarget, vp.GetConstructionPlane(), vp.Name
    old_layer = doc.Layers.CurrentLayerIndex
    ids, layers, groups, geometry = [], [], [], []
    witness = None
    views = {}
    sdk = {}

    def xyz(point):
        return [float(point.X), float(point.Y), float(point.Z)]

    def view_value():
        info = R.DocObjects.ViewportInfo(vp)
        try:
            return dict(perspective=bool(vp.IsPerspectiveProjection),location=xyz(vp.CameraLocation),
                        target=xyz(vp.CameraTarget),direction=xyz(vp.CameraDirection),up=xyz(vp.CameraUp),
                        frustum=[float(getattr(info,'Frustum'+key)) for key in ['Left','Right','Bottom','Top','Near','Far']])
        finally:
            info.Dispose()

    def capture(stage):
        if case.get('view_trace'):
            views[stage] = view_value()
        if stage == 'after' and case.get('sdk_checks'):
            info = R.DocObjects.ViewportInfo(vp)
            try:
                def outlines(mesh, frame=None):
                    values = mesh.GetOutlines(frame) if frame is not None else mesh.GetOutlines(info,plane)
                    return [[xyz(p)for p in poly]for poly in values or []]
                direction = G.Vector3d(vp.CameraDirection)
                direction.Unitize()
                eye = vp.CameraLocation
                depth = (2./(1./info.FrustumNear+1./info.FrustumFar) if vp.IsPerspectiveProjection
                         else (info.FrustumNear+info.FrustumFar)*.5)
                normal = G.Vector3d(-direction.X,-direction.Y,-direction.Z)
                output_plane = G.Plane(G.Point3d(eye.X+depth*direction.X,eye.Y+depth*direction.Y,eye.Z+depth*direction.Z),normal)
                aligned = not vp.IsPerspectiveProjection and abs(abs(direction.X*plane.Normal.X+direction.Y*plane.Normal.Y+direction.Z*plane.Normal.Z)-1.)<1e-12
                if aligned:
                    output_plane = plane
                for index, identifier in enumerate(ids):
                    mesh = doc.Objects.FindId(identifier).Geometry
                    if not isinstance(mesh,G.Mesh):
                        continue
                    projected = G.Mesh()
                    projected.Vertices.UseDoublePrecisionVertices = True
                    try:
                        for i in range(mesh.Vertices.Count):
                            p = mesh.Vertices.Point3dAt(i)
                            if vp.IsPerspectiveProjection:
                                dx,dy,dz = p.X-eye.X,p.Y-eye.Y,p.Z-eye.Z
                                t = depth/(dx*direction.X+dy*direction.Y+dz*direction.Z)
                                q = G.Point3d(eye.X+t*dx,eye.Y+t*dy,eye.Z+t*dz)
                            else:
                                q = output_plane.ClosestPoint(p)
                            projected.Vertices.Add(q)
                        for face in mesh.Faces:
                            if face.IsTriangle:projected.Faces.AddFace(face.A,face.B,face.C)
                            else:projected.Faces.AddFace(face.A,face.B,face.C,face.D)
                        sdk[str(index)] = dict(original=outlines(geometry[index]),admitted=outlines(mesh),
                                               projected=outlines(projected,output_plane),
                                               vertex_normals=int(mesh.Normals.Count),face_normals=int(mesh.FaceNormals.Count))
                    finally:
                        projected.Dispose()
            finally:
                info.Dispose()
        return snapshot()

    def make(data):
        kind = data['kind']
        if kind == 'mesh':
            mesh = G.Mesh()
            mesh.Vertices.UseDoublePrecisionVertices = True
            for point in data['vertices']:
                mesh.Vertices.Add(*point)
            for face in data['faces']:
                mesh.Faces.AddFace(*face)
            return mesh
        if kind == 'point':
            return G.Point(G.Point3d(*data['point']))
        if kind == 'brep_box':
            return G.Brep.CreateFromBox(G.BoundingBox(G.Point3d(0.,0.,1.),G.Point3d(4.,3.,4.)))
        if kind == 'sphere':
            return G.Sphere(G.Point3d(0.,0.,3.), 2.).ToBrep()
        if kind == 'plane':
            return G.PlaneSurface(G.Plane(G.Point3d(0.,0.,2.),G.Vector3d.ZAxis),G.Interval(0.,4.),G.Interval(0.,3.)).ToNurbsSurface()
        raise ValueError('unsupported fixed outline source')

    def snapshot():
        result = []
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            a, g = obj.Attributes, obj.Geometry
            row = dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                       role='source' if obj.Id in ids else 'witness' if obj.Id == witness else 'output',
                       selected=bool(obj.IsSelected(False)),name=a.Name,layer=doc.Layers[a.LayerIndex].Name,
                       groups=sorted(doc.Groups.GroupName(i) for i in (a.GetGroupList() or [])),
                       color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],
                       color_source=int(a.ColorSource),crc=int(g.DataCRC(0)))
            if isinstance(g,G.Mesh):
                row['mesh'] = host['_polygon_mesh_value'](g)
            elif isinstance(g,G.Point):
                row['point'] = xyz(g.Location)
            elif isinstance(g,G.Curve):
                ok, polyline = g.TryGetPolyline()
                if not ok:
                    raise ValueError('outline command output is not a polyline')
                row['polyline'] = [xyz(p) for p in polyline]
            else:
                row['kind'] = str(g.GetType().Name)
            result.append(row)
        return result

    try:
        record = doc.BeginUndoRecord('Owned outline source setup')
        try:
            for index in range(len(case['sources'])+1):
                layer = R.DocObjects.Layer()
                layer.Name = 'Outline source '+str(index) if index < len(case['sources']) else 'Outline output'
                layer.Color = S.Drawing.Color.FromArgb(91,109,121)
                layers.append(doc.Layers.Add(layer))
                layer.Dispose()
            for index, data in enumerate(case['sources']):
                g = make(data)
                geometry.append(g)
                if not g.IsValid:
                    raise ValueError('invalid outline command source')
                a = R.DocObjects.ObjectAttributes()
                try:
                    a.LayerIndex, a.Name = layers[index], 'Source '+str(index)
                    a.ObjectColor = S.Drawing.Color.FromArgb(13+index,27,53)
                    a.ColorSource = R.DocObjects.ObjectColorSource.ColorFromObject
                    identifier = doc.Objects.Add(g,a)
                    if identifier == S.Guid.Empty:
                        raise ValueError('outline source admission failed')
                    ids.append(identifier)
                finally:
                    a.Dispose()
            for index, members in enumerate([[identifier] for identifier in ids]+[ids]):
                groups.append(doc.Groups.Add('Outline group '+str(index),members))
            doc.Layers.SetCurrentLayerIndex(layers[-1],True)
        finally:
            doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True)
        record = doc.BeginUndoRecord('Owned outline Undo witness')
        try:
            a = R.DocObjects.ObjectAttributes()
            a.Name, a.LayerIndex = 'Undo witness', layers[-1]
            witness = doc.Objects.AddPoint(G.Point3d(99.,98.,97.),a)
            a.Dispose()
        finally:
            doc.EndUndoRecord(record)
        projection = (R.Display.DefinedViewportProjection.Front if case['view']=='front'
                      else R.Display.DefinedViewportProjection.Perspective if case['view']=='perspective'
                      else R.Display.DefinedViewportProjection.Top)
        vp.SetProjection(projection,'Owned MeshOutline command',False)
        if case['view'] in ('oblique','perspective'):
            vp.SetCameraLocations(G.Point3d(2.,1.5,2.),G.Point3d(12.,16.5,22.))
            vp.CameraUp = G.Vector3d.ZAxis
        data = case['plane']
        plane = G.Plane(G.Point3d(*data['origin']),G.Vector3d(*data['x']),G.Vector3d(*data['y']))
        vp.SetConstructionPlane(plane)
        info = R.DocObjects.ViewportInfo(vp)
        try:
            if not info.SetFrustum(*case['frustum']):
                raise ValueError('outline command frustum rejected')
            vp.SetViewProjection(info,False)
            view = dict(perspective=bool(vp.IsPerspectiveProjection),location=xyz(vp.CameraLocation),
                        target=xyz(vp.CameraTarget),direction=xyz(vp.CameraDirection),up=xyz(vp.CameraUp),
                        frustum=[float(getattr(info,'Frustum'+key)) for key in ['Left','Right','Bottom','Top','Near','Far']])
        finally:
            info.Dispose()
        selected = case.get('selected',list(range(len(ids))))
        if case['preselect']:
            for index in selected:
                doc.Objects.Select(ids[index])
        model = R.FileIO.File3dm()
        try:
            layer = model.Layers.AddLayer('Default',S.Drawing.Color.Black)
            for identifier in ids:
                obj = doc.Objects.FindId(identifier)
                a = obj.Attributes.Duplicate()
                a.LayerIndex = layer
                a.RemoveFromAllGroups()
                model.Objects.Add(obj.Geometry,a)
                a.Dispose()
            model.Settings.ModelAbsoluteTolerance = doc.ModelAbsoluteTolerance
            source_3dm = S.Convert.ToBase64String(model.ToByteArray())
        finally:
            model.Dispose()
        before = capture('before')
        macro = '_-MeshOutline'
        if not case['preselect']:
            macro += ' '+' '.join('_SelID '+str(ids[index]) for index in selected)
        macro += ' _Cancel' if case.get('cancel') else ' _Enter _Cancel'
        marker = 'Owned MeshOutline '+str(S.Guid.NewGuid())
        R.RhinoApp.WriteLine(marker)
        success, after, events = observe_command(R.Commands.Command,'MeshOutline',
            lambda:R.RhinoApp.RunScript(macro,True),lambda:capture('after'),lambda:[],True)
        history = R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        after_script = capture('after_script')
        render_meshes = []
        for identifier in ids:
            obj = doc.Objects.FindId(identifier)
            if isinstance(obj.Geometry,G.Mesh):
                render_meshes.append([host['_polygon_mesh_value'](obj.Geometry)])
            elif isinstance(obj.Geometry,G.Brep) or isinstance(obj.Geometry,G.Surface):
                meshes = obj.GetMeshes(G.MeshType.Render)
                render_meshes.append([host['_polygon_mesh_value'](m) for m in meshes or []])
            else:
                render_meshes.append([])
        R.RhinoApp.RunScript('_Undo',False)
        undo = capture('undo')
        R.RhinoApp.RunScript('_Redo',False)
        redo = capture('redo')
        result = dict(input=case,view=view,plane=dict(origin=xyz(plane.Origin),x=xyz(plane.XAxis),y=xyz(plane.YAxis),z=xyz(plane.ZAxis)),
                    before=before,after=after,after_script=after_script,undo=undo,redo=redo,
                    success=success,events=events,history=history,source_3dm=source_3dm,render_meshes=render_meshes)
        if case.get('view_trace'):
            result['views'] = views
        if case.get('sdk_checks'):
            result['sdk_checks'] = sdk
        return result,0
    finally:
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True)
        for index in reversed(groups):
            doc.Groups.Delete(index)
        for index in reversed(layers):
            doc.Layers.Delete(index,True)
        vp.SetViewProjection(old_view,False)
        vp.SetCameraTarget(old_target,False)
        vp.SetConstructionPlane(old_plane)
        vp.Name = old_name
        old_view.Dispose()
        for g in geometry:
            g.Dispose()
