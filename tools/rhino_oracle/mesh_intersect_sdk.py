"""Public intersection API observations on disposable mesh duplicates."""


def capture(meshes, host, tolerance):
    R, S = host['Rhino'], host['System']
    G, api = R.Geometry, R.Geometry.Intersect.Intersection
    xyz = lambda p: [float(p.X),float(p.Y),float(p.Z)]
    polylines = lambda curves: [[xyz(p)for p in curve]for curve in curves or []]
    result = {}
    for name in ['fast','accurate','modern']:
        copies=[m.DuplicateMesh()for m in meshes]
        overlap_mesh=None
        try:
            if name == 'fast':
                lines=api.MeshMeshFast(copies[0],copies[1])
                result[name]=dict(segments=[[xyz(l.From),xyz(l.To)]for l in lines or []])
            elif name == 'accurate':
                result[name]=dict(polylines=polylines(api.MeshMeshAccurate(copies[0],copies[1],tolerance)))
            else:
                ok,curves,overlaps,overlap_mesh=api.MeshMesh(S.Array[G.Mesh](copies),tolerance,
                    True,False,None,S.Threading.CancellationToken(),None)
                result[name]=dict(success=bool(ok),polylines=polylines(curves),
                    coplanar=polylines(overlaps),mesh=host['_polygon_mesh_value'](overlap_mesh)if overlap_mesh else None)
        except Exception as error:
            result[name]=dict(error=str(error))
        finally:
            if overlap_mesh is not None:overlap_mesh.Dispose()
            for mesh in copies:mesh.Dispose()
    return result
