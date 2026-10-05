"""Closed public SDK surface/UV image and Pushup witnesses."""
import re

CASES = ('warped_exact', 'warped_offset', 'warped_bump', 'rational_speed',
         'tensor_cross', 'tensor_dyadic', 'transformed', 'negative_gauge',
         'cylinder_quarter', 'cylinder_knot_cross', 'sphere_oblique',
         'sphere_seam', 'torus_diagonal')


def validate_request(q):
    if (type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations', 1)) is not int or q.get('iterations', 1) != 1
            or not isinstance(q.get('operations'), list) or not 1 <= len(q['operations']) <= 32):
        raise ValueError('surface curve image requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
                or op['op'] != 'surface_curve_image' or not isinstance(op['id'], str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen or op['case'] not in CASES):
            raise ValueError('invalid surface curve image recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='surface_curve_image', id='surface_image_'+case, case=case)
        for case in CASES])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino = host['Rhino']
    if Rhino.Commands.Command.InCommand():
        raise ValueError('surface curve image requires idle execution')
    if list(Rhino.RhinoDoc.ActiveDoc.Objects):
        raise ValueError('surface curve image requires an empty owned document')
    G = Rhino.Geometry
    case = op['case']
    owned = []

    def keep(g):
        if g is None or not g.IsValid:
            if g is not None:
                g.Dispose()
            raise ValueError('surface image source is invalid')
        owned.append(g)
        return g

    def cp(point, weight=1.):
        return dict(point=list(point), weight=weight)

    def curve(points, weights=None, degree=1, domain=(0., 1.)):
        weights = weights or [1.]*len(points)
        return dict(degree=degree, control_points=[cp(p, w) for p, w in zip(points, weights)],
                    knots=[domain[0]]*(degree+1)+[domain[1]]*(degree+1))

    def transform(point):
        x, y, z = point
        return (2*x-y+.5*z+4, .25*x+3*y-.5*z-2, .5*x+.75*y+4*z+7)

    try:
        uv_points = [(0., 0., 0.), (1., 1., 0.)]
        uv_weights = None
        uv_domain = (0., 1.)
        source = None
        pushup = case.startswith(('cylinder', 'sphere', 'torus'))
        if pushup:
            if case.startswith('cylinder'):
                surface = keep(G.Cylinder(G.Circle(G.Plane.WorldXY, 2.), 3.).ToNurbsSurface())
                uv_points = [(1.2, 1.5, 0.), (1.9, 1.5, 0.)] if case.endswith('cross') else [(0.2, .25, 0.), (1.2, 2.5, 0.)]
            elif case.startswith('sphere'):
                surface = keep(G.Sphere(G.Point3d.Origin, 2.).ToNurbsSurface())
                uv_points = [(2.9, .1, 0.), (3.3, .3, 0.)] if case.endswith('seam') else [(.2, .1, 0.), (1.2, .8, 0.)]
            else:
                surface = keep(G.Torus(G.Plane.WorldXY, 4., 1.).ToNurbsSurface())
                uv_points = [(.2, .1, 0.), (1.2, 1., 0.)]
        else:
            points = [(0., 0., 0.), (1., 0., 0.), (0., 1., 0.), (1., 1., 1.)]
            weights = [1.]*4
            count_u, knots_u = 2, [0., 0., 1., 1.]
            if case.startswith('tensor'):
                knot = .3 if case.endswith('cross') else .5
                points = [(u, v, 0.) for v in (0., 1.) for u in (0., knot, 1.)]
                weights = [1.]*6
                count_u, knots_u = 3, [0., 0., knot, 1., 1.]
                uv_points = [(0., .25, 0.), (1., .25, 0.)]
                source = curve([(0., .25, 0.), (1., .25, 0.)])
            elif case == 'rational_speed':
                points[-1] = (1., 1., 0.)
                weights = [1., 2., 1., 2.]
                uv_points = [(0., .25, 0.), (1., .25, 0.)]
                uv_weights = [2., 1.]
                source = curve([(0., .25, 0.), (1., .25, 0.)])
            else:
                offset = 1./1024 if case == 'warped_offset' else 0.
                middle = offset + (1./128 if case == 'warped_bump' else 0.)
                source = curve([(0., 0., offset), (.5, .5, middle), (1., 1., 1.+offset)], degree=2)
                if case == 'transformed':
                    points = [transform(p) for p in points]
                    source['control_points'] = [cp(transform(c['point'])) for c in source['control_points']]
                    uv_domain = (2., 5.)
                if case == 'negative_gauge':
                    weights = [-2.]*4
                    uv_weights = [-3., -3.]
                    source['control_points'] = [cp(c['point'], -4.) for c in source['control_points']]
            definition = dict(degree_u=1, degree_v=1, control_point_count_u=count_u,
                              control_point_count_v=2, control_points=[cp(p,w) for p,w in zip(points,weights)],
                              knots_u=knots_u, knots_v=[0., 0., 1., 1.])
            surface = keep(host['_nurbs_surface_from_definition'](definition))
        uv = keep(host['_nurbs_curve_from_definition'](curve(uv_points, uv_weights, domain=uv_domain), 2))
        before = (host['_nurbs_surface_definition'](surface), host['_nurbs_curve_definition'](uv))
        spatial = keep(surface.Pushup(uv, 1e-6)) if pushup else keep(host['_nurbs_curve_from_definition'](source))
        spatial_nurbs = keep(spatial.ToNurbsCurve())
        samples = []
        for i in range(129):
            fraction = i/128.
            p = uv.PointAt(uv.Domain.ParameterAt(fraction))
            image = surface.PointAt(p.X, p.Y)
            reference = spatial.PointAt(spatial.Domain.ParameterAt(fraction))
            closest = spatial.ClosestPoint(image)
            if closest is None or not closest[0]:
                raise ValueError('surface image closest point failed')
            samples.append(dict(fraction=fraction, uv=host['_xyz'](p)[:2], image=host['_xyz'](image),
                                spatial=host['_xyz'](reference), error=float(image.DistanceTo(reference)),
                                locus_error=float(image.DistanceTo(spatial.PointAt(closest[1])))))
        after = (host['_nurbs_surface_definition'](surface), host['_nurbs_curve_definition'](uv))
        return dict(surface=before[0], parameter_curve=before[1], spatial_curve=host['_nurbs_curve_definition'](spatial_nurbs),
                    sources_unchanged=before == after, pushup=pushup, pushup_tolerance=1e-6,
                    samples=samples, maximum_sample_error=max(p['error'] for p in samples),
                    maximum_locus_error=max(p['locus_error'] for p in samples)), 0
    finally:
        for g in reversed(owned):
            g.Dispose()
