"""Public winding APIs, isolated command scenes and fresh-mesh runtime controls."""
try:
    from .unify_normals_cases import CASE_GROUPS, validate
except (ImportError, ValueError):
    from unify_normals_cases import CASE_GROUPS, validate


def run(op, host, iterations, tolerance):
    validate(op)
    case = next(cases[op['case']] for cases in CASE_GROUPS.values() if op['case'] in cases)
    if op['op'] == 'unify_normals_command':
        from crease_probe import run_scene
        return run_scene(case, host, iterations, tolerance)
    R, S = host['Rhino'], host['System']
    G = R.Geometry
    if (iterations != 1 or list(R.RhinoDoc.ActiveDoc.Objects)
            or R.RhinoDoc.ActiveDoc.UndoRecordingIsActive):
        raise ValueError('owned empty idle document required')
    from mesh_channel_capture import snapshot

    def build(data):
        mesh = G.Mesh()
        try:
            mesh.Vertices.UseDoublePrecisionVertices = True
            for p in data['vertices']:
                mesh.Vertices.Add(*p)
            for f in data['faces']:
                mesh.Faces.AddFace(*f)
            for r, g, b, a in data.get('colors', []):
                mesh.VertexColors.Add(S.Drawing.Color.FromArgb(255 - a, r, g, b))
            for uv in data.get('uv', []):
                mesh.TextureCoordinates.Add(*uv)
            for n in data.get('normals', []):
                mesh.Normals.Add(*n)
            for n in data.get('ngons', []):
                mesh.Ngons.AddNgon(G.MeshNgon.Create(S.Array[S.UInt32](n['vertices']),
                                                    S.Array[S.UInt32](n['faces'])))
            if data.get('compute_face_normals'):
                mesh.FaceNormals.ComputeFaceNormals()
            return mesh
        except Exception:
            mesh.Dispose()
            raise

    def value(mesh):
        v = snapshot(mesh, host, {'transparency': []})
        v['face_normals'] = [[float(n.X), float(n.Y), float(n.Z)] for n in mesh.FaceNormals]
        if case.get('runtime_checks'):
            v['face_edges'] = [[int(e) for e in mesh.TopologyEdges.GetEdgesForFace(f)]
                               for f in range(mesh.Faces.Count)]
        return v

    def fresh_trial(mesh, count_first):
        try:
            before = value(mesh)
            predicted = int(mesh.UnifyNormals(True)) if count_first else None
            changed = int(mesh.UnifyNormals(False))
            return dict(before=before, predicted=predicted, changed=changed, after=value(mesh))
        finally:
            mesh.Dispose()

    mesh = build(case)
    try:
        before = value(mesh)
        predicted = int(mesh.UnifyNormals(True))
        after_count = value(mesh)
        changed = int(mesh.UnifyNormals(False) if case.get('boolean_overload') else mesh.UnifyNormals())
        after = value(mesh)
        runtime = None
        if case.get('runtime_checks'):
            runtime = dict(raw=fresh_trial(build(after), False),
                           counted_raw=fresh_trial(build(after), True),
                           duplicate=fresh_trial(mesh.DuplicateMesh(), False))
        second = int(mesh.UnifyNormals(False) if case.get('boolean_overload') else mesh.UnifyNormals())
        result = dict(input=case, before=before, predicted=predicted, after_count=after_count,
                      changed=changed, after=after, second=second, after_second=value(mesh))
        if runtime is not None:
            result['runtime'] = runtime
        return result, 0
    finally:
        mesh.Dispose()
