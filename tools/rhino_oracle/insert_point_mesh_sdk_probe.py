"""Compare public triangulation and quad conversion on fixed captured regions."""
try:
    from .insert_point_mesh_sdk_cases import CASES, validate
except (ImportError, ValueError):
    from insert_point_mesh_sdk_cases import CASES, validate


def run(op, host, iterations, tolerance):
    validate(op)
    R = host['Rhino']; G = R.Geometry
    if iterations != 1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.RhinoDoc.ActiveDoc.UndoRecordingIsActive:
        raise ValueError('owned empty idle document required')
    case = CASES[op['case']]
    if op.get('methods'):
        from insert_point_mesh_methods import run as methods
        return methods(case, host), 0
    points = [G.Point3d(*p) for p in case['vertices']]
    if op.get('origins'):
        results={}
        for i in range(len(points)):
            rotated=points[i:]+points[:i]
            polyline=G.Polyline(rotated+[rotated[0]])
            mesh=G.Mesh();mesh.Vertices.UseDoublePrecisionVertices=True
            for p in rotated:mesh.Vertices.Add(p)
            for f in polyline.TriangulateClosedPolyline():mesh.Faces.AddFace(f)
            try:
                triangles=host['_polygon_mesh_value'](mesh)
                mesh.Faces.ConvertTrianglesToQuads(0.01,0.)
                results[str(i)]=dict(triangles=triangles,quads=host['_polygon_mesh_value'](mesh))
            finally:mesh.Dispose()
        return dict(input=case,origins=results),0
    polyline = G.Polyline(points+[points[0]])
    meshes = {}
    mesh = G.Mesh.CreateFromClosedPolyline(polyline)
    if mesh is not None:meshes['closed_polyline'] = mesh
    triangles = polyline.TriangulateClosedPolyline()
    if triangles:
        mesh = G.Mesh();mesh.Vertices.UseDoublePrecisionVertices = True
        for p in points:mesh.Vertices.Add(p)
        for f in triangles:mesh.Faces.AddFace(f)
        meshes['polyline_triangles'] = mesh
    results = {}
    try:
        for name, mesh in meshes.items():
            results[name] = host['_polygon_mesh_value'](mesh)
            for ratio in [0., 0.5]:
                copy = mesh.DuplicateMesh()
                try:
                    changed = copy.Faces.ConvertTrianglesToQuads(0.01, ratio)
                    results[name+'_quads_'+str(ratio)] = dict(changed=bool(changed), mesh=host['_polygon_mesh_value'](copy))
                finally:copy.Dispose()
        return dict(input=case, results=results), 0
    finally:
        for mesh in meshes.values():mesh.Dispose()
