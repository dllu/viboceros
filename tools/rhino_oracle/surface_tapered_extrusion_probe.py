"""Public SDK recipes for independent surface taper inputs; capture pending."""
import copy
import math


def validate(operation):
    required = {'op', 'id', 'source', 'face', 'displacement', 'draft_angle', 'solid', 'round_corners'}
    if (not isinstance(operation, dict) or set(operation) != required
            or operation['op'] != 'surface_tapered_extrusion'
            or type(operation['face']) is not int or operation['face'] < 0
            or type(operation['solid']) is not bool or type(operation['round_corners']) is not bool
            or not isinstance(operation['source'], dict)
            or operation['source'].get('type') not in ('surface', 'brep', 'box_brep')):
        raise ValueError('invalid surface taper fields or options')
    d, angle = operation['displacement'], operation['draft_angle']
    if (not isinstance(d, list) or len(d) != 3
            or any(type(x) not in (int, float) or not math.isfinite(x) for x in d)
            or not 0 < math.hypot(*d) < float('inf')
            or type(angle) not in (int, float) or not math.isfinite(angle) or abs(angle) >= 90):
        raise ValueError('invalid surface taper displacement or angle')


def request():
    try:
        from .surface_extrusion_probe import request as surface_request
    except ImportError:
        from surface_extrusion_probe import request as surface_request
    q = surface_request()
    ring = next(o['source'] for o in q['operations'] if o['id'] == 'surface_ring_hole')
    def patch(bump):
        return dict(type='surface', degree_u=2, degree_v=2, control_point_count_u=3, control_point_count_v=3,
                    knots_u=[0.,0.,0.,1.,1.,1.], knots_v=[0.,0.,0.,1.,1.,1.],
                    control_points=[dict(point=[i*3.-3.,j*3.-3.,1. if bump and i==1 and j==1 else 0.], weight=1.) for j in range(3) for i in range(3)])
    warped = dict(type='surface', degree_u=1, degree_v=1, control_point_count_u=2, control_point_count_v=2,
                  knots_u=[0.,0.,1.,1.], knots_v=[0.,0.,1.,1.],
                  control_points=[dict(point=p, weight=1.) for p in [[0.,0.,0.],[6.,0.,0.],[0.,6.,0.],[6.,6.,1.]]])
    cases = [('planar', patch(False), 2., 10., False, True),
             ('planar_down', patch(False), -2., 10., False, True),
             ('bump', patch(True), 2., 10., False, True),
             ('bump_down', patch(True), -2., 10., False, True),
             ('ring_inner', ring, 2., 10., False, True),
             ('ring_outer', ring, 2., -10., False, True),
             ('ring_walls', ring, 2., 10., False, False),
             ('round_outward', patch(False), 2., -10., True, True),
             ('zero_warped', warped, 2., 0., False, True),
             ('nonplanar_walls', warped, 2., 10., False, False),
             ('shifted_bump', patch(True), 2., 10., False, True)]
    q['operations'] = [dict(op='surface_tapered_extrusion', id='surface_taper_'+name, source=copy.deepcopy(source),
                            face=0, displacement=[0.,0.,h], draft_angle=angle, round_corners=round_corners, solid=solid)
                       for name, source, h, angle, round_corners, solid in cases]
    for cp in q['operations'][-1]['source']['control_points']:
        cp['point'] = [a+b for a,b in zip(cp['point'], [10000.,-30000.,50000.])]
    q['tolerance']['absolute'] = 1e-5
    return q


def run(operation, host, iterations, tolerance):
    validate(operation)
    G = host['Rhino'].Geometry
    source = host['_object_source'](operation['source'], tolerance)
    brep = None
    try:
        if isinstance(source, G.Surface): brep = source.ToBrep()
        elif isinstance(source, G.Brep): brep = source.DuplicateBrep()
        if brep is None or operation['face'] >= brep.Faces.Count:
            raise ValueError('invalid surface taper face')
        vector = host['_vector'](operation['displacement'])
        direction = host['_vector'](operation['displacement'])
        height = float(direction.Length)
        if not direction.Unitize(): raise ValueError('invalid surface taper direction')
        angle, absolute = operation['draft_angle'], float(tolerance['absolute'])
        def compute():
            owned = []
            def keep(value):
                if value is None: raise ValueError('SDK surface taper construction failed')
                owned.append(value)
                return value
            try:
                if angle == 0:
                    path = keep(G.LineCurve(G.Point3d.Origin, G.Point3d.Origin+vector))
                    result = keep(brep.Faces[operation['face']].CreateExtrusion(path, operation['solid']))
                else:
                    cap = keep(brep.Faces[operation['face']].DuplicateFace(False))
                    parts = [cap] if operation['solid'] else []
                    corner = G.ExtrudeCornerType.Round if operation['round_corners'] else G.ExtrudeCornerType.Sharp
                    for loop in cap.Loops:
                        if loop.LoopType not in (G.BrepLoopType.Outer, G.BrepLoopType.Inner):
                            raise ValueError('unsupported surface taper loop')
                        curves = []
                        for trim in loop.Trims:
                            if trim.Edge is None: continue
                            curve = keep(trim.Edge.DuplicateCurve())
                            if trim.IsReversed() and not curve.Reverse():
                                raise ValueError('could not orient surface taper trim')
                            curves.append(curve)
                        joined_curves = G.Curve.JoinCurves(curves, absolute, True)
                        for curve in joined_curves or []: keep(curve)
                        if not joined_curves or len(joined_curves) != 1 or not joined_curves[0].IsClosed:
                            raise ValueError('surface taper requires closed boundary chains')
                        curve = joined_curves[0]
                        if curve.ClosedCurveOrientation(G.Plane(curve.PointAtStart, direction)) == G.CurveOrientation.Clockwise:
                            if not curve.Reverse(): raise ValueError('could not normalize taper profile orientation')
                        draft = -angle if loop.LoopType == G.BrepLoopType.Inner else angle
                        walls = G.Brep.CreateFromTaperedExtrude(curve, height, direction, curve.PointAtStart,
                            math.radians(draft), corner, absolute, float(tolerance['angular']))
                        if not walls: raise ValueError('SDK surface taper returned no walls')
                        for wall in walls: parts.append(keep(wall))
                    joined = G.Brep.JoinBreps(parts, absolute)
                    for b in joined or []: keep(b)
                    if not joined: raise ValueError('SDK surface taper join failed')
                    if operation['solid']:
                        if len(joined) != 1: raise ValueError('SDK surface taper base did not join')
                        result = keep(joined[0].CapPlanarHoles(absolute))
                    elif len(joined) == 1:
                        result = joined[0]
                    else:
                        result = keep(G.Brep())
                        for b in joined: result.Append(b)
                if not result.IsValid or operation['solid'] and not result.IsSolid:
                    raise ValueError('SDK surface taper is invalid or did not close')
                volume = None
                if result.IsSolid:
                    mass = keep(G.VolumeMassProperties.Compute(result)); volume = abs(float(mass.Volume))
                bounds = result.GetBoundingBox(True)
                return dict(breps=[dict(is_solid=bool(result.IsSolid), face_count=int(result.Faces.Count),
                    volume=volume, bounds=[host['_xyz'](bounds.Min), host['_xyz'](bounds.Max)])])
            finally:
                disposed = set()
                for value in reversed(owned):
                    if id(value) not in disposed: value.Dispose(); disposed.add(id(value))
        return host['_measure'](iterations, compute)
    finally:
        if brep is not None: brep.Dispose()
        source.Dispose()
