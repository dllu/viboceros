"""Public Mesh.GetOutlines overloads with owned viewport state restoration."""
try:
    from .mesh_outline_cases import ALL_CASES, validate
except (ImportError, ValueError):
    from mesh_outline_cases import ALL_CASES, validate


def run(operation, host, iterations, tolerance):
    validate(operation)
    R, G = host['Rhino'], host['Rhino'].Geometry
    doc = R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or doc.UndoRecordingIsActive:
        raise ValueError('owned empty idle document required')
    case = ALL_CASES[operation['case']]
    mesh = G.Mesh()
    viewport = doc.Views.ActiveView.ActiveViewport
    original = R.DocObjects.ViewportInfo(viewport)
    old_plane = viewport.GetConstructionPlane()
    old_name = viewport.Name
    old_target = viewport.CameraTarget
    old_tolerance = doc.ModelAbsoluteTolerance

    def xyz(p):
        return [float(p.X), float(p.Y), float(p.Z)]

    def plane_value(p):
        return dict(origin=xyz(p.Origin), x=xyz(p.XAxis), y=xyz(p.YAxis), z=xyz(p.ZAxis))

    try:
        if 'document_tolerance' in case:
            doc.ModelAbsoluteTolerance = float(case['document_tolerance'])
        mesh.Vertices.UseDoublePrecisionVertices = True
        for point in case['mesh']['vertices']:
            mesh.Vertices.Add(*point)
        for face in case['mesh']['faces']:
            mesh.Faces.AddFace(*face)
        if not mesh.IsValid:
            raise ValueError('invalid outline source mesh')
        data = case['plane']
        plane = G.Plane(G.Point3d(*data['origin']), G.Vector3d(*data['x']), G.Vector3d(*data['y']))
        before = int(mesh.DataCRC(0))
        info = None
        try:
            if case['method'] != 'plane':
                view = case['view']
                projection = (R.Display.DefinedViewportProjection.Front if view == 'front'
                              else R.Display.DefinedViewportProjection.Perspective if view == 'perspective'
                              else R.Display.DefinedViewportProjection.Top)
                viewport.SetProjection(projection, 'Owned MeshOutline input', False)
                if view in ('oblique', 'perspective'):
                    camera = case.get('camera', dict(target=[2., 1.5, 2.],
                                                   location=[12., 16.5, 22.], up=[0., 0., 1.]))
                    viewport.SetCameraLocations(G.Point3d(*camera['target']), G.Point3d(*camera['location']))
                    viewport.CameraUp = G.Vector3d(*camera['up'])
                viewport.SetConstructionPlane(plane)
                info = R.DocObjects.ViewportInfo(viewport)
                if 'frustum' in case:
                    if not info.SetFrustum(*case['frustum']):
                        raise ValueError('fixed outline frustum rejected')
                    viewport.SetViewProjection(info, False)
            if case['method'] == 'plane':
                outlines = mesh.GetOutlines(plane)
            elif case['method'] == 'viewport':
                outlines = mesh.GetOutlines(viewport)
            else:
                outlines = mesh.GetOutlines(info, plane)
            result = dict(input=case, plane=plane_value(plane), source_unchanged=before == int(mesh.DataCRC(0)),
                          returned_null=outlines is None,
                          outlines=[[xyz(point) for point in polyline] for polyline in (outlines or [])])
            if operation['case'].startswith('outline_scale_'):
                result['document_tolerance'] = float(doc.ModelAbsoluteTolerance)
            if info is not None:
                result['view'] = dict(perspective=bool(viewport.IsPerspectiveProjection),
                                      location=xyz(viewport.CameraLocation), target=xyz(viewport.CameraTarget),
                                      direction=xyz(viewport.CameraDirection), up=xyz(viewport.CameraUp),
                                      frustum=[float(getattr(info, 'Frustum' + name))
                                               for name in ['Left', 'Right', 'Bottom', 'Top', 'Near', 'Far']])
                if case.get('capture_transform'):
                    matrix = viewport.GetTransform(R.DocObjects.CoordinateSystem.World,
                                                   R.DocObjects.CoordinateSystem.Clip)
                    result['view']['world_to_clip'] = [
                        [float(getattr(matrix, 'M' + str(i) + str(j))) for j in range(4)] for i in range(4)]
            return result, 0
        finally:
            if info is not None:
                info.Dispose()
    finally:
        doc.ModelAbsoluteTolerance = old_tolerance
        viewport.SetViewProjection(original, False)
        viewport.SetCameraTarget(old_target, False)
        viewport.SetConstructionPlane(old_plane)
        viewport.Name = old_name
        original.Dispose()
        mesh.Dispose()
