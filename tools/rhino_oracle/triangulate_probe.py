"""Public ConvertQuadsToTriangles and TriangulateMesh in an owned document."""
try:
    from .triangulate_cases import SDK,COMMANDS,HOLDOUTS,validate
except (ImportError,ValueError):
    from triangulate_cases import SDK,COMMANDS,HOLDOUTS,validate


def run(op,host,iterations,tolerance):
    validate(op)
    if op['op']=='triangulate_mesh_command':
        from crease_probe import run_scene
        cases=HOLDOUTS if op['case']in HOLDOUTS else COMMANDS
        return run_scene(cases[op['case']],host,iterations,tolerance)
    R,S=host['Rhino'],host['System'];G=R.Geometry
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects)or R.RhinoDoc.ActiveDoc.UndoRecordingIsActive:raise ValueError('owned empty idle document required')
    from mesh_channel_capture import snapshot
    case=SDK[op['case']];m=G.Mesh();m.Vertices.UseDoublePrecisionVertices=True
    try:
        for p in case['vertices']:m.Vertices.Add(*p)
        for f in case['faces']:m.Faces.AddFace(*f)
        for r,g,b,a in case.get('colors',[]):m.VertexColors.Add(S.Drawing.Color.FromArgb(255-a,r,g,b))
        for uv in case.get('uv',[]):m.TextureCoordinates.Add(*uv)
        for n in case.get('normals',[]):m.Normals.Add(*n)
        for n in case.get('ngons',[]):m.Ngons.AddNgon(G.MeshNgon.Create(S.Array[S.UInt32](n['vertices']),S.Array[S.UInt32](n['faces'])))
        before=snapshot(m,host,{'transparency':[]});count=m.Faces.QuadCount
        success=m.Faces.ConvertQuadsToTriangles()
        return dict(input=case,before=before,after=snapshot(m,host,{'transparency':[]}),success=bool(success),converted=int(count-m.Faces.QuadCount)),0
    finally:m.Dispose()
