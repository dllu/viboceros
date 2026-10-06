"""Closed SDK Pullback/isocurve records and independent endpoint witnesses."""
import math
import re

CASES = ('planar_forward', 'planar_reverse', 'cylinder_forward', 'cylinder_reverse',
         'sphere_forward', 'sphere_reverse', 'torus_forward', 'torus_reverse')


def validate_request(q):
    if (type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations', 1)) is not int or q.get('iterations', 1) != 1
            or not isinstance(q.get('operations'), list) or not 1 <= len(q['operations']) <= 32):
        raise ValueError('pullback endpoints requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
                or op['op'] != 'surface_pullback_endpoints' or not isinstance(op['id'], str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen or op['case'] not in CASES):
            raise ValueError('invalid pullback endpoints recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='surface_pullback_endpoints', id='pullback_endpoints_'+case, case=case)
        for case in CASES])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino = host['Rhino']
    if Rhino.Commands.Command.InCommand():
        raise ValueError('pullback endpoints requires idle execution')
    if list(Rhino.RhinoDoc.ActiveDoc.Objects):
        raise ValueError('pullback endpoints requires an empty owned document')
    G = Rhino.Geometry
    owned = []

    def keep(g):
        if g is None or not g.IsValid:
            if g is not None:
                g.Dispose()
            raise ValueError('pullback source is invalid')
        owned.append(g)
        return g

    def cp(point):
        return dict(point=list(point), weight=1.)

    def xyz(p):
        return [p.X, p.Y, p.Z]

    try:
        case = op['case']
        limit = 1e-6
        if case.startswith('planar'):
            surface = keep(host['_nurbs_surface_from_definition'](dict(
                degree_u=1, degree_v=1, control_point_count_u=2, control_point_count_v=2,
                control_points=[cp(p) for p in ((0., 0., 0.), (1., 0., 0.), (0., 1., 0.), (.75, 1., 0.))],
                knots_u=[0., 0., 1., 1.], knots_v=[0., 0., 1., 1.])))
            spatial = keep(host['_nurbs_curve_from_definition'](dict(degree=2,
                control_points=[cp(p) for p in ((0., 0., 0.), (7./16., 0., 0.), (7./8., .5, 0.))],
                knots=[2., 2., 2., 5., 5., 5.])))
            a, b = 1./28., 9./448.
            t = ((3*b-4*a)+math.sqrt((3*b-4*a)**2+40*a*b))/(10*b)
            limit = t*t*(1-t)**2*(a+b*t)*1.0001
            endpoints = [[0., 0.], [1., .5+limit/4.]]
        else:
            if case.startswith('cylinder'):
                surface = keep(G.Cylinder(G.Circle(G.Plane.WorldXY, 2.), 3.).ToNurbsSurface())
                v = 1.5
            elif case.startswith('sphere'):
                surface = keep(G.Sphere(G.Point3d.Origin, 2.).ToNurbsSurface())
                v = 0.
            else:
                surface = keep(G.Torus(G.Plane.WorldXY, 4., 1.).ToNurbsSurface())
                v = 0.
            iso = keep(surface.IsoCurve(0, v))
            spatial = keep(iso.ToNurbsCurve())
            endpoints = [[surface.Domain(0).T0, v], [surface.Domain(0).T1, v]]
        if case.endswith('reverse'):
            if not spatial.Reverse():
                raise ValueError('spatial reversal failed')
            endpoints.reverse()
        before = (host['_nurbs_surface_definition'](surface), host['_nurbs_curve_definition'](spatial))
        pullback = surface.Pullback(spatial, limit)
        parameter_curve = None
        if pullback is not None:
            keep(pullback)
            parameter_curve = keep(pullback.ToNurbsCurve())
        samples = []
        for i in range(129):
            fraction = i/128.
            model = spatial.PointAt(spatial.Domain.ParameterAt(fraction))
            sample = dict(fraction=fraction, spatial_point=xyz(model))
            if parameter_curve is not None:
                uv = parameter_curve.PointAt(parameter_curve.Domain.ParameterAt(fraction))
                image = surface.PointAt(uv.X, uv.Y)
                ok, t = spatial.ClosestPoint(image)
                if not ok:
                    raise ValueError('pullback locus witness failed')
                sample.update(parameter_point=[uv.X, uv.Y], surface_point=xyz(image),
                              error=image.DistanceTo(model), locus_error=image.DistanceTo(spatial.PointAt(t)))
            samples.append(sample)
        endpoint_samples = []
        for uv, t in zip(endpoints, (spatial.Domain.T0, spatial.Domain.T1)):
            point = surface.PointAt(uv[0], uv[1])
            model = spatial.PointAt(t)
            endpoint_samples.append(dict(parameter_point=uv, surface_point=xyz(point), spatial_point=xyz(model),
                                         error=point.DistanceTo(model)))
        after = (host['_nurbs_surface_definition'](surface), host['_nurbs_curve_definition'](spatial))
        return dict(surface=before[0], spatial_curve=before[1], endpoints=endpoints, limit=limit,
                    native_pullback_succeeded=parameter_curve is not None,
                    native_parameter_curve=host['_nurbs_curve_definition'](parameter_curve) if parameter_curve is not None else None,
                    samples=samples, endpoint_samples=endpoint_samples, sources_unchanged=before == after), 0
    finally:
        for g in reversed(owned):
            g.Dispose()
