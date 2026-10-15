"""Public RhinoCommon parallel silhouette recipes; captures remain pending.

The SDK returns requested silhouette classes, without a command history or
visibility-parity claim. Keep its raw curve order and parameterization for audit.
"""
import re

CASES = ('box_top', 'box_bottom', 'box_oblique', 'box_internal',
         'sphere_top', 'sphere_oblique', 'torus_top')


def validate(operation):
    if (not isinstance(operation, dict) or set(operation) != {'op', 'id', 'case'}
            or operation['op'] != 'silhouette_parallel'
            or not isinstance(operation['id'], str)
            or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', operation['id']) is None
            or operation['case'] not in CASES):
        raise ValueError('invalid parallel silhouette recipe')


def request():
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-5, relative=1e-12, angular=0.05),
                operations=[dict(op='silhouette_parallel', id='silhouette_'+case, case=case)
                            for case in CASES])


def run(operation, host, iterations, tolerance):
    validate(operation)
    G = host['Rhino'].Geometry
    case = operation['case']
    if case.startswith('box_'):
        source = G.Brep.CreateFromBox(G.BoundingBox(G.Point3d(0., 0., 0.), G.Point3d(2., 3., 4.)))
    elif case.startswith('sphere_'):
        source = G.Sphere(G.Point3d.Origin, 2.).ToBrep()
    else:
        source = G.Torus(G.Plane.WorldXY, 3., 1.).ToBrep()
    if source is None:
        raise ValueError('could not construct silhouette source')
    # Rhino's camera direction points into the scene; our Direction points to the eye.
    toward_eye = ((1., 2., 3.) if case in ('box_oblique', 'box_internal', 'sphere_oblique')
                  else (0., 0., -1.) if case == 'box_bottom' else (0., 0., 1.))
    camera = G.Vector3d(*[-x for x in toward_eye])
    if not camera.Unitize():
        source.Dispose()
        raise ValueError('invalid silhouette camera direction')
    T = G.SilhouetteType
    types = T.Projecting | T.TangentProjects | T.Tangent | T.Crease | T.Boundary
    if case == 'box_internal':
        types = types | T.NonSilhouetteCrease | T.NonSilhouetteTangent | T.NonSilhouetteSeam

    def compute():
        results = G.Silhouette.Compute(source, types, camera, float(tolerance['absolute']), 0.05)
        curves = []
        try:
            for item in results or []:
                curve = item.Curve
                if curve is None:
                    raise ValueError('missing silhouette curve')
                nurbs = curve.ToNurbsCurve()
                try:
                    curves.append(dict(length=float(curve.GetLength()),
                        samples=[host['_xyz'](curve.PointAt(curve.Domain.ParameterAt(i/32.)))
                                 for i in range(33)], definition=host['_nurbs_curve_definition'](nurbs)))
                finally:
                    if nurbs is not None:
                        nurbs.Dispose()
            return dict(curves=curves, total_length=sum(c['length'] for c in curves))
        finally:
            for item in results or []:
                if item.Curve is not None:
                    item.Curve.Dispose()
    try:
        return host['_measure'](iterations, compute)
    finally:
        source.Dispose()
