"""Public static Mesh.RebuildMesh and owned RebuildMesh command workflows."""
try:
    from .rebuild_mesh_cases import SDK,DISCOVERY,COMMANDS,HOLDOUTS,validate
except (ImportError,ValueError):
    from rebuild_mesh_cases import SDK,DISCOVERY,COMMANDS,HOLDOUTS,validate


def run(op,host,iterations,tolerance):
    validate(op)
    if op['op']=='rebuild_mesh_command':
        from crease_probe import run_scene
        cases=DISCOVERY if op['case']in DISCOVERY else COMMANDS if op['case']in COMMANDS else HOLDOUTS
        return run_scene(cases[op['case']],host,iterations,tolerance)
    R,S=host['Rhino'],host['System'];G=R.Geometry
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects)or R.RhinoDoc.ActiveDoc.UndoRecordingIsActive:raise ValueError('owned empty idle document required')
    from mesh_channel_capture import snapshot
    case=SDK[op['case']];m=G.Mesh();m.Vertices.UseDoublePrecisionVertices=True;rebuilt=None
    try:
        for p in case['vertices']:m.Vertices.Add(*p)
        for f in case['faces']:m.Faces.AddFace(*f)
        for r,g,b,a in case.get('colors',[]):m.VertexColors.Add(S.Drawing.Color.FromArgb(255-a,r,g,b))
        for uv in case.get('uv',[]):m.TextureCoordinates.Add(*uv)
        for n in case.get('normals',[]):m.Normals.Add(*n)
        for n in case.get('ngons',[]):m.Ngons.AddNgon(G.MeshNgon.Create(S.Array[S.UInt32](n['vertices']),S.Array[S.UInt32](n['faces'])))
        before=snapshot(m,host,{'transparency':[]})
        rebuilt=G.Mesh.RebuildMesh(m)if case['options']is None else G.Mesh.RebuildMesh(m,*case['options'])
        return dict(input=case,before=before,source_after=snapshot(m,host,{'transparency':[]}),after=snapshot(rebuilt,host,{'transparency':[]})if rebuilt is not None else None,success=rebuilt is not None),0
    finally:
        if rebuilt is not None:rebuilt.Dispose()
        m.Dispose()
