"""Public selective quad triangulation in an owned idle document."""
try:
    from .nonplanar_quad_cases import CASES, validate
except (ImportError, ValueError):
    from nonplanar_quad_cases import CASES, validate


def run(op,host,iterations,tolerance):
    validate(op)
    R,S=host['Rhino'],host['System'];G=R.Geometry
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects)or R.RhinoDoc.ActiveDoc.UndoRecordingIsActive:
        raise ValueError('owned empty idle document required')
    from mesh_channel_capture import snapshot
    case=CASES[op['case']];mesh=G.Mesh();mesh.Vertices.UseDoublePrecisionVertices=True
    try:
        for p in case['vertices']:mesh.Vertices.Add(*p)
        for f in case['faces']:mesh.Faces.AddFace(*f)
        for r,g,b,a in case.get('colors',[]):mesh.VertexColors.Add(S.Drawing.Color.FromArgb(255-a,r,g,b))
        for uv in case.get('uv',[]):mesh.TextureCoordinates.Add(*uv)
        for n in case.get('normals',[]):mesh.Normals.Add(*n)
        for n in case.get('ngons',[]):mesh.Ngons.AddNgon(G.MeshNgon.Create(S.Array[S.UInt32](n['vertices']),S.Array[S.UInt32](n['faces'])))
        before=snapshot(mesh,host,{'transparency':[]})
        distance=R.RhinoMath.UnsetValue if case['distance']is None else case['distance']
        angle=R.RhinoMath.UnsetValue if case['angle']is None else case['angle']
        converted=mesh.Faces.ConvertNonPlanarQuadsToTriangles(distance,angle,case['method'])
        return dict(input=case,before=before,after=snapshot(mesh,host,{'transparency':[]}),converted=int(converted)),0
    finally:mesh.Dispose()
