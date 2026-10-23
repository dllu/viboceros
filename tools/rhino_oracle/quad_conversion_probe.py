"""Public mesh conversion in an owned empty idle document; no command emulation."""
try:
    from .quad_conversion_cases import CASES, validate
except (ImportError, ValueError):
    from quad_conversion_cases import CASES, validate


def run(op, host, iterations, tolerance):
    validate(op)
    R,S=host['Rhino'],host['System'];G=R.Geometry
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects)or R.RhinoDoc.ActiveDoc.UndoRecordingIsActive:
        raise ValueError('owned empty idle document required')
    from mesh_channel_capture import snapshot
    case=CASES[op['case']];mesh=G.Mesh();mesh.Vertices.UseDoublePrecisionVertices=True
    try:
        for p in case['vertices']:mesh.Vertices.Add(*p)
        for f in case['faces']:mesh.Faces.AddFace(*f)
        for r,g,b,a in case.get('colors',[]):mesh.VertexColors.Add(S.Drawing.Color.FromArgb(a,r,g,b))
        for uv in case.get('uv',[]):mesh.TextureCoordinates.Add(*uv)
        for n in case.get('normals',[]):mesh.Normals.Add(*n)
        for n in case.get('ngons',[]):
            mesh.Ngons.AddNgon(G.MeshNgon.Create(S.Array[S.UInt32](n['vertices']),S.Array[S.UInt32](n['faces'])))
        before=snapshot(mesh,host,{'transparency':[]});valid_before=bool(mesh.IsValid)
        changed=mesh.Faces.ConvertTrianglesToQuads(case['angle'],case['ratio'])
        return dict(input=case,before=before,after=snapshot(mesh,host,{'transparency':[]}),changed=bool(changed),valid_before=valid_before,valid_after=bool(mesh.IsValid)),0
    finally:mesh.Dispose()
