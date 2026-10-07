"""Closed public SDK pullbacks across seams, singular ends, and changed charts."""
import re

FAMILIES = ('cylinder_u', 'cylinder_u_relocated', 'sphere_u', 'sphere_pole_u', 'sphere_v',
            'sphere_v_relocated', 'sphere_v_swapped', 'torus_u', 'torus_v',
            'two_poles', 'two_poles_swapped', 'singular_diagonal')
CASES = tuple(family + '_' + direction for family in FAMILIES
              for direction in ('forward', 'reverse'))


def validate_request(q):
    if (type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations', 1)) is not int or q.get('iterations', 1) != 1
            or not isinstance(q.get('operations'), list) or not 1 <= len(q['operations']) <= 32):
        raise ValueError('linear pullback requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
                or op['op'] != 'surface_pullback_linear' or not isinstance(op['id'], str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen or op['case'] not in CASES):
            raise ValueError('invalid linear pullback recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='surface_pullback_linear', id='pullback_linear_' + case, case=case)
        for case in CASES])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino, System = host['Rhino'], host['System']
    if Rhino.Commands.Command.InCommand():
        raise ValueError('linear pullback requires idle execution')
    if list(Rhino.RhinoDoc.ActiveDoc.Objects):
        raise ValueError('linear pullback requires an empty owned document')
    G = Rhino.Geometry
    owned = []

    def keep(g):
        if g is None or not g.IsValid:
            if g is not None:
                g.Dispose()
            raise ValueError('invalid linear pullback source')
        owned.append(g)
        return g

    def cp(p):
        return dict(point=list(p), weight=1.)

    def xyz(p):
        return [p.X, p.Y, p.Z]

    try:
        family, direction = op['case'].rsplit('_', 1)
        if family.startswith('two_poles'):
            surface = keep(host['_nurbs_surface_from_definition'](dict(
                degree_u=1, degree_v=2, control_point_count_u=2, control_point_count_v=3,
                control_points=[cp(p) for p in ((0.,0.,0.),(0.,0.,0.),(0.,2.,.5),
                                               (2.,2.,.5),(0.,0.,1.),(0.,0.,1.))],
                knots_u=[0.,0.,1.,1.], knots_v=[0.,0.,0.,1.,1.,1.])))
            spatial = keep(host['_nurbs_curve_from_definition'](dict(degree=2,
                control_points=[cp(p) for p in ((0.,0.,0.),(.75,2.,.5),(0.,0.,1.))],
                knots=[0.,0.,0.,1.,1.,1.])))
            endpoints = [[.375,0.],[.375,1.]]
            if family.endswith('_swapped'):
                transposed = keep(surface.Transpose())
                surface = keep(transposed.ToNurbsSurface())
                endpoints = [p[::-1] for p in endpoints]
        elif family == 'singular_diagonal':
            surface = keep(host['_nurbs_surface_from_definition'](dict(
                degree_u=2, degree_v=1, control_point_count_u=3, control_point_count_v=2,
                control_points=[cp(p) for p in ((0.,0.,0.),(0.,0.,0.),(1.,0.,0.),
                                               (0.,1.,0.),(0.,1.,.5),(1.,1.,1.))],
                knots_u=[0.,0.,0.,1.,1.,1.], knots_v=[0.,0.,1.,1.])))
            spatial = keep(host['_nurbs_curve_from_definition'](dict(degree=2,
                control_points=[cp(p) for p in ((0.,0.,0.),(0.,.5,0.),(1.,1.,1.))],
                knots=[0.,0.,0.,1.,1.,1.])))
            endpoints = [[0.,0.],[1.,1.]]
        else:
            if family.startswith('cylinder'):
                surface = keep(G.Cylinder(G.Circle(G.Plane.WorldXY, 2.), 3.).ToNurbsSurface())
            elif family.startswith('sphere'):
                surface = keep(G.Sphere(G.Point3d.Origin, 2.).ToNurbsSurface())
            else:
                surface = keep(G.Torus(G.Plane.WorldXY, 4., 1.).ToNurbsSurface())
            if family.endswith('_relocated'):
                if not surface.SetDomain(0, G.Interval(-7., 19.)) or not surface.SetDomain(1, G.Interval(-3., 6.)):
                    raise ValueError('surface domain change failed')
            along_u = not ('sphere_v' in family or family == 'torus_v')
            fraction = .375 if family.startswith('sphere_v') else .5 if family.startswith('cylinder') or family == 'sphere_u' else 0.
            constant = surface.Domain(1 if along_u else 0).ParameterAt(fraction)
            if family.endswith('_swapped'):
                transposed = keep(surface.Transpose())
                surface = keep(transposed.ToNurbsSurface())
                along_u = not along_u
            iso = keep(surface.IsoCurve(0 if along_u else 1, constant))
            spatial = keep(iso.ToNurbsCurve())
            varying = surface.Domain(0 if along_u else 1)
            endpoints = [[varying.T0, constant], [varying.T1, constant]] if along_u else [
                [constant, varying.T0], [constant, varying.T1]]
        spatial.Domain = G.Interval(2., 5.)
        if direction == 'reverse':
            if not spatial.Reverse():
                raise ValueError('curve reversal failed')
            endpoints.reverse()
        before = (host['_nurbs_surface_definition'](surface), host['_nurbs_curve_definition'](spatial))
        limit = 1e-6
        native = surface.Pullback(spatial, limit)
        parameter_curve = None
        if native is not None:
            keep(native)
            parameter_curve = keep(native.ToNurbsCurve())
        samples = []
        for i in range(129):
            f = i / 128.
            model = spatial.PointAt(spatial.Domain.ParameterAt(f))
            uv = [(1-f)*endpoints[0][axis] + f*endpoints[1][axis] for axis in (0, 1)]
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
        # Retain bounded SDK timing separately: the native geometric fitting
        # contract differs from the local complete correspondence certificate.
        for _ in range(2):
            warm = surface.Pullback(spatial, limit)
            if warm is not None:
                warm.Dispose()
        successes = 0
        watch = System.Diagnostics.Stopwatch.StartNew()
        for _ in range(16):
            candidate = surface.Pullback(spatial, limit)
            if candidate is not None:
                successes += int(candidate.IsValid)
                candidate.Dispose()
        watch.Stop()
        benchmark = dict(iterations=16, successes=successes,
                         elapsed_ns=int(watch.ElapsedTicks * 1e9 / System.Diagnostics.Stopwatch.Frequency))
        after = (host['_nurbs_surface_definition'](surface), host['_nurbs_curve_definition'](spatial))
        return dict(surface=before[0], spatial_curve=before[1], limit=limit,
                    reference_endpoints=endpoints, native_pullback_succeeded=parameter_curve is not None,
                    native_parameter_curve=host['_nurbs_curve_definition'](parameter_curve) if parameter_curve is not None else None,
                    samples=samples, sources_unchanged=before == after, benchmark=benchmark), 0
    finally:
        for g in reversed(owned):
            g.Dispose()
