"""Public SDK comparison for independent tapered extrusion inputs; capture pending."""
import math


def validate(operation):
    required = {'op', 'id', 'curve', 'displacement', 'draft_angle', 'solid', 'round_corners'}
    if not isinstance(operation, dict) or set(operation) != required or operation['op'] != 'tapered_extrusion':
        raise ValueError('invalid tapered extrusion fields')
    direction = operation['displacement']
    angle = operation['draft_angle']
    if (not isinstance(direction, list) or len(direction) != 3
            or any(type(x) not in (int, float) or not math.isfinite(x) for x in direction)
            or math.hypot(*direction) == 0 or not math.isfinite(math.hypot(*direction))
            or type(angle) not in (int, float) or not math.isfinite(angle) or abs(angle) >= 90
            or type(operation['solid']) is not bool or type(operation['round_corners']) is not bool
            or not isinstance(operation['curve'], dict)
            or not isinstance(operation['curve'].get('control_points'), list)
            or not 2 <= len(operation['curve']['control_points']) <= 100000):
        raise ValueError('invalid tapered extrusion geometry or options')


def request():
    weights = [1., math.sqrt(0.5), 1., math.sqrt(0.5), 1., math.sqrt(0.5), 1., math.sqrt(0.5), 1.]
    points = [(3., 0., 0.), (3., 3., 0.), (0., 3., 0.), (-3., 3., 0.), (-3., 0., 0.),
              (-3., -3., 0.), (0., -3., 0.), (3., -3., 0.), (3., 0., 0.)]
    def curve(points, degree, knots, weights=None):
        return dict(degree=degree, knots=knots, control_points=[dict(point=list(p), weight=w)
                    for p, w in zip(points, weights or [1.]*len(points))])
    circle = curve(points, 2, [0., 0., 0., 1., 1., 2., 2., 3., 3., 4., 4., 4.], weights)
    rotated = curve([(5., y, -x) for x, y, z in points], 2, circle['knots'], weights)
    rectangle = curve([(-3., -3., 0.), (3., -3., 0.), (3., 3., 0.), (-3., 3., 0.), (-3., -3., 0.)],
                      1, [0., 0., 1., 2., 3., 4., 4.])
    line = curve([(0., 0., 0.), (4., 0., 0.)], 1, [0., 0., 1., 1.])
    cases = [('circle_inward', circle, [0., 0., 2.], 10., False, True),
             ('circle_outward', circle, [0., 0., 2.], -10., False, True),
             ('circle_down', circle, [0., 0., -2.], 10., False, True),
             ('circle_rotated', rotated, [2., 0., 0.], 10., False, True),
             ('rectangle_sharp', rectangle, [0., 0., 2.], 10., False, True),
             ('rectangle_round', rectangle, [0., 0., 2.], -10., True, True),
             ('open_line', line, [0., 0., 2.], 10., False, False)]
    return dict(protocol_version=1, iterations=1, tolerance=dict(absolute=1e-5, relative=1e-12, angular=1e-8),
                operations=[dict(op='tapered_extrusion', id='tapered_'+name, curve=c, displacement=d,
                                 draft_angle=a, round_corners=r, solid=s) for name, c, d, a, r, s in cases])


def run(operation, host, iterations, tolerance):
    validate(operation)
    G = host['Rhino'].Geometry
    source = host['_nurbs_curve_from_definition'](operation['curve'])
    try:
        direction = host['_vector'](operation['displacement'])
        height = float(direction.Length)
        if not direction.Unitize():
            raise ValueError('invalid tapered extrusion direction')
        corner = G.ExtrudeCornerType.Round if operation['round_corners'] else G.ExtrudeCornerType.Sharp
        absolute = float(tolerance['absolute'])
        def compute():
            owned = []
            try:
                breps = G.Brep.CreateFromTaperedExtrude(source, height, direction, source.PointAtStart,
                    math.radians(operation['draft_angle']), corner, absolute, float(tolerance['angular']))
                owned.extend(breps or [])
                if not owned:
                    raise ValueError('SDK produced no tapered extrusion')
                records = []
                for brep in breps:
                    if operation['solid'] and source.IsClosed:
                        capped = brep.CapPlanarHoles(absolute)
                        if capped is None:
                            raise ValueError('SDK could not cap tapered extrusion')
                        owned.append(capped)
                        brep = capped
                    bounds = brep.GetBoundingBox(True)
                    volume = None
                    if brep.IsSolid:
                        mass = G.VolumeMassProperties.Compute(brep)
                        if mass is None:
                            raise ValueError('SDK volume integration failed')
                        try:
                            volume = abs(float(mass.Volume))
                        finally:
                            mass.Dispose()
                    records.append(dict(is_solid=bool(brep.IsSolid), face_count=int(brep.Faces.Count),
                        volume=volume, bounds=[host['_xyz'](bounds.Min), host['_xyz'](bounds.Max)]))
                return dict(breps=records)
            finally:
                for brep in owned:
                    brep.Dispose()
        return host['_measure'](iterations, compute)
    finally:
        source.Dispose()
