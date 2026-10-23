"""Fixed public API alternatives; these use captured output regions only."""


def run(case, host):
    G, S = host['Rhino'].Geometry, host['System']
    points = [G.Point3d(*p) for p in case['vertices']]
    closed = points+[points[0]]
    curves = [G.LineCurve(points[i], points[(i+1) % len(points)]) for i in range(len(points))]
    boundary = G.PolylineCurve(closed)
    constraints = S.Collections.Generic.List[S.Collections.Generic.IEnumerable[G.Point3d]]()
    constraints.Add(S.Collections.Generic.List[G.Point3d](closed))
    parameters = G.MeshingParameters.Default
    parameters.SimplePlanes = True
    parameters.RefineGrid = False
    parameters.GridMinCount = 0
    parameters.GridMaxCount = 0
    parameters.MinimumEdgeLength = 0.
    parameters.MaximumEdgeLength = 0.
    parameters.Tolerance = 1e-8
    parameters.RelativeTolerance = 0.
    methods = [
        ('tessellation_free', lambda: G.Mesh.CreateFromTessellation(points, None, G.Plane.WorldXY, False)),
        ('tessellation_boundary', lambda: G.Mesh.CreateFromTessellation(points, constraints, G.Plane.WorldXY, False)),
        ('lines_4', lambda: G.Mesh.CreateFromLines(S.Array[G.Curve](curves), 4, 1e-8)),
        ('lines_6', lambda: G.Mesh.CreateFromLines(S.Array[G.Curve](curves), 6, 1e-8)),
        ('planar_default', lambda: G.Mesh.CreateFromPlanarBoundary(boundary, G.MeshingParameters.Default, 1e-8)),
        ('planar_simple', lambda: G.Mesh.CreateFromPlanarBoundary(boundary, parameters, 1e-8)),
    ]
    results = {}
    try:
        for name, create in methods:
            mesh = converted = None
            try:
                mesh = create()
                if mesh is None:
                    results[name] = dict(mesh=None, quads=None)
                    continue
                value = host['_polygon_mesh_value'](mesh)
                converted = mesh.DuplicateMesh()
                changed = converted.Faces.ConvertTrianglesToQuads(.01, 0.)
                results[name] = dict(mesh=value, changed=bool(changed),
                                     quads=host['_polygon_mesh_value'](converted))
            except Exception as error:
                results[name] = dict(error=type(error).__name__+': '+str(error))
            finally:
                if converted is not None:converted.Dispose()
                if mesh is not None:mesh.Dispose()
    finally:
        parameters.Dispose()
        boundary.Dispose()
        for curve in curves:curve.Dispose()
    return dict(input=case, methods=results)
