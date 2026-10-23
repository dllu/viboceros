"""Public closed-polyline triangulation and relaxed 3D quad conversion diagnostics."""


def run(case, host):
    G = host['Rhino'].Geometry
    points = [G.Point3d(*p) for p in case['vertices']]
    results = {}
    for origin in range(len(points)):
        ring = points[origin:]+points[:origin]
        polyline = G.Polyline(ring+[ring[0]])
        methods = [('closed', lambda: G.Mesh.CreateFromClosedPolyline(polyline))]
        def triangles():
            faces = polyline.TriangulateClosedPolyline()
            if not faces:return None
            mesh = G.Mesh()
            mesh.Vertices.UseDoublePrecisionVertices = True
            for p in ring:mesh.Vertices.Add(p)
            for f in faces:mesh.Faces.AddFace(f)
            return mesh
        methods.append(('triangles', triangles))
        for name, create in methods:
            mesh = None
            key = name+'_'+str(origin)
            try:
                mesh = create()
                if mesh is None:
                    results[key] = dict(mesh=None, quads=None)
                    continue
                value = host['_polygon_mesh_value'](mesh)
                changed = mesh.Faces.ConvertTrianglesToQuads(1.5707963267948966, 0.)
                results[key] = dict(mesh=value, quads=host['_polygon_mesh_value'](mesh), changed=bool(changed))
            except Exception as error:
                results[key] = dict(error=type(error).__name__+': '+str(error))
            finally:
                if mesh is not None:mesh.Dispose()
    return dict(input=case, variants=results)
