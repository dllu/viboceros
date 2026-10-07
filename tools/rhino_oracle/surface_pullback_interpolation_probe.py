"""Closed public SDK recipes for nonlinear pullbacks with singular endpoints."""
import math
import re

FAMILIES = ('singular_quadratic', 'singular_cubic', 'two_poles_quadratic',
            'two_poles_quadratic_swapped', 'sqrt_boundary')
CASES = tuple(family + '_' + direction for family in FAMILIES
              for direction in ('forward', 'reverse'))


def validate_request(q):
    if (type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations', 1)) is not int or q.get('iterations', 1) != 1
            or not isinstance(q.get('operations'), list) or not 1 <= len(q['operations']) <= 32):
        raise ValueError('interpolated pullback requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
                or op['op'] != 'surface_pullback_interpolation' or not isinstance(op['id'], str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen or op['case'] not in CASES):
            raise ValueError('invalid interpolated pullback recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='surface_pullback_interpolation', id='pullback_interpolation_' + case, case=case)
        for case in CASES])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino = host['Rhino']
    if Rhino.Commands.Command.InCommand():
        raise ValueError('interpolated pullback requires idle execution')
    if list(Rhino.RhinoDoc.ActiveDoc.Objects):
        raise ValueError('interpolated pullback requires an empty owned document')
    owned = []

    def keep(g):
        if g is None or not g.IsValid:
            if g is not None:
                g.Dispose()
            raise ValueError('invalid interpolated pullback source')
        owned.append(g)
        return g

    def cp(p):
        return dict(point=list(p), weight=1.)

    def xyz(p):
        return [p.X, p.Y, p.Z]

    try:
        family, direction = op['case'].rsplit('_', 1)
        if family.startswith('two_poles'):
            surface_definition = dict(degree_u=1, degree_v=2, control_point_count_u=2,
                control_point_count_v=3, knots_u=[0.,0.,1.,1.], knots_v=[0.,0.,0.,1.,1.,1.],
                control_points=[cp(p) for p in ((0.,0.,0.),(0.,0.,0.),(0.,2.,.5),
                                               (2.,2.,.5),(0.,0.,1.),(0.,0.,1.))])
            degree = 4
            points = ((0.,0.,0.),(.25,1.,.25),(1./3.,4./3.,.5),(.75,1.,.75),(0.,0.,1.))
            reference = lambda t: [.25+.5*t*t, t]
        else:
            surface_definition = dict(degree_u=2, degree_v=1, control_point_count_u=3,
                control_point_count_v=2, knots_u=[0.,0.,0.,1.,1.,1.], knots_v=[0.,0.,1.,1.],
                control_points=[cp(p) for p in ((0.,0.,0.),(0.,0.,0.),(1.,0.,0.),
                                               (0.,1.,0.),(0.,1.,.5),(1.,1.,1.))])
            if family == 'singular_quadratic':
                degree = 3
                points = ((0.,0.,0.),(0.,0.,0.),(1./3.,1./3.,0.),(1.,1.,1.))
                reference = lambda t: [t, t*t]
            elif family == 'singular_cubic':
                degree = 4
                points = ((0.,0.,0.),(0.,0.,0.),(1./6.,0.,0.),(.5,.25,0.),(1.,1.,1.))
                reference = lambda t: [t, t*t*t]
            else:
                degree = 1
                points = ((0.,0.,0.),(1.,0.,0.))
                reference = lambda t: [math.sqrt(t), 0.]
        surface = keep(host['_nurbs_surface_from_definition'](surface_definition))
        spatial = keep(host['_nurbs_curve_from_definition'](dict(degree=degree,
            control_points=[cp(p) for p in points], knots=[2.]*(degree+1)+[5.]*(degree+1))))
        if family.endswith('_swapped'):
            transposed = keep(surface.Transpose())
            surface = keep(transposed.ToNurbsSurface())
            reference = lambda t, base=reference: base(t)[::-1]
        if direction == 'reverse':
            if not spatial.Reverse():
                raise ValueError('curve reversal failed')
            reference = lambda t, base=reference: base(1-t)
        before = (host['_nurbs_surface_definition'](surface), host['_nurbs_curve_definition'](spatial))
        limit = 1e-6
        native = surface.Pullback(spatial, limit)
        parameter_curve = None
        if native is not None:
            keep(native)
            parameter_curve = keep(native.ToNurbsCurve())
        samples = []
        for i in range(129):
            f = i/128.
            model = spatial.PointAt(spatial.Domain.ParameterAt(f))
            uv = reference(f)
            image = surface.PointAt(uv[0], uv[1])
            row = dict(fraction=f, spatial_point=xyz(model), reference_parameter_point=uv,
                       reference_surface_point=xyz(image), reference_error=image.DistanceTo(model))
            if parameter_curve is not None:
                uv = parameter_curve.PointAt(parameter_curve.Domain.ParameterAt(f))
                image = surface.PointAt(uv.X, uv.Y)
                ok, t = spatial.ClosestPoint(image)
                if not ok:
                    raise ValueError('native pullback locus witness failed')
                row.update(native_parameter_point=[uv.X, uv.Y], native_surface_point=xyz(image),
                           native_error=image.DistanceTo(model), native_locus_error=image.DistanceTo(spatial.PointAt(t)))
            samples.append(row)
        after = (host['_nurbs_surface_definition'](surface), host['_nurbs_curve_definition'](spatial))
        return dict(surface=before[0], spatial_curve=before[1], limit=limit,
                    native_pullback_succeeded=parameter_curve is not None,
                    native_parameter_curve=host['_nurbs_curve_definition'](parameter_curve) if parameter_curve is not None else None,
                    samples=samples, sources_unchanged=before == after), 0
    finally:
        for g in reversed(owned):
            g.Dispose()
