"""Independent public face-extrusion fixtures; licensed capture remains pending."""
import copy
import math


def validate(operation):
    if set(operation) != {'op', 'id', 'source', 'face', 'displacement', 'solid', 'both_sides'} or operation['op'] != 'surface_extrusion':
        raise ValueError('invalid surface extrusion fields')
    v = operation['displacement']
    if (not isinstance(v, list) or len(v) != 3
            or any(type(x) not in (int, float) or not math.isfinite(x) for x in v)
            or not 0 < math.hypot(*v) < float('inf')
            or type(operation['face']) is not int or operation['face'] < 0
            or type(operation['solid']) is not bool or type(operation['both_sides']) is not bool
            or not isinstance(operation['source'], dict)
            or operation['source'].get('type') not in ('surface', 'brep', 'box_brep')):
        raise ValueError('invalid surface extrusion source or options')


def request():
    def surface(warp):
        return dict(degree_u=1, degree_v=1, control_point_count_u=2, control_point_count_v=2,
                    knots_u=[0., 0., 1., 1.], knots_v=[0., 0., 1., 1.],
                    control_points=[dict(point=list(p), weight=1.) for p in
                                    [(0., 0., 0.), (2., 0., 0.), (0., 3., 0.), (2., 3., warp)]])
    plane, warped = dict(type='surface', **surface(0.)), dict(type='surface', **surface(1.))
    def circle(radius, reverse=False):
        points = [(1., 0., 0.), (1., 1., 0.), (0., 1., 0.), (-1., 1., 0.), (-1., 0., 0.),
                  (-1., -1., 0.), (0., -1., 0.), (1., -1., 0.), (1., 0., 0.)]
        if reverse:
            points.reverse()
        return dict(degree=2, knots=[0., 0., 0., 1., 1., 2., 2., 3., 3., 4., 4., 4.],
                    control_points=[dict(point=[radius*x, radius*y, z], weight=1. if i % 2 == 0 else math.sqrt(0.5))
                                    for i, (x, y, z) in enumerate(points)])
    carrier = dict(degree_u=1, degree_v=1, control_point_count_u=2, control_point_count_v=2,
                   knots_u=[-3., -3., 3., 3.], knots_v=[-3., -3., 3., 3.],
                   control_points=[dict(point=list(p), weight=1.) for p in
                                   [(-3., -3., 0.), (3., -3., 0.), (-3., 3., 0.), (3., 3., 0.)]])
    boundaries = [dict(curve=c, parameter_curve=copy.deepcopy(c)) for c in (circle(3.), circle(1., True))]
    ring = dict(type='brep', surface=carrier, boundaries=boundaries, interior_uv=[2., 0.])
    cases = [('planar_up', plane, [0., 0., 2.], True, False),
             ('planar_down', plane, [0., 0., -2.], True, False),
             ('warped_up', warped, [0., 0., 2.], True, False),
             ('warped_oblique', warped, [1., 0., 2.], True, False),
             ('warped_both', warped, [0., 0., 2.], True, True),
             ('ring_hole', ring, [0., 0., 2.], True, False),
             ('ring_walls', ring, [0., 0., 2.], False, False),
             ('box_face', dict(type='box_brep', min=[0., 0., 0.], max=[1., 1., 1.]), [2., 2., 2.], True, False)]
    return dict(protocol_version=1, iterations=1, tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-8),
                operations=[dict(op='surface_extrusion', id='surface_'+name, source=source, face=0,
                                 displacement=v, solid=solid, both_sides=both) for name, source, v, solid, both in cases])


def run(operation, host, iterations, tolerance):
    validate(operation)
    G = host['Rhino'].Geometry
    source = host['_object_source'](operation['source'], tolerance)
    brep = None
    try:
        if isinstance(source, G.Surface):
            brep = source.ToBrep()
        elif isinstance(source, G.Brep):
            brep = source.DuplicateBrep()
        if brep is None or operation['face'] >= brep.Faces.Count:
            raise ValueError('invalid surface extrusion face')
        vector = host['_vector'](operation['displacement'])
        if operation['both_sides']:
            if not brep.Translate(-vector):
                raise ValueError('could not shift both-sided extrusion source')
            vector *= 2.
        def compute():
            path = G.LineCurve(G.Point3d.Origin, G.Point3d.Origin + vector)
            result = None
            try:
                result = brep.Faces[operation['face']].CreateExtrusion(path, operation['solid'])
                if result is None or not result.IsValid:
                    raise ValueError('SDK surface extrusion failed')
                volume = None
                if result.IsSolid:
                    mass = G.VolumeMassProperties.Compute(result)
                    if mass is None:
                        raise ValueError('SDK surface extrusion volume failed')
                    try:
                        volume = abs(float(mass.Volume))
                    finally:
                        mass.Dispose()
                bounds = result.GetBoundingBox(True)
                return dict(breps=[dict(is_solid=bool(result.IsSolid), face_count=int(result.Faces.Count),
                    volume=volume, bounds=[host['_xyz'](bounds.Min), host['_xyz'](bounds.Max)])])
            finally:
                if result is not None:
                    result.Dispose()
                path.Dispose()
        return host['_measure'](iterations, compute)
    finally:
        if brep is not None:
            brep.Dispose()
        source.Dispose()
