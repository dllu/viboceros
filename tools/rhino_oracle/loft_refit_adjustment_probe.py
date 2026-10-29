"""Public nine-station length batch and a common tangent-handle correction."""
import math


def inspect(curve, host, fractional_tolerance=1e-4, nonrational=False, local_seed=None):
    geometry = host['Rhino'].Geometry
    xyz = host['_xyz']
    start, end = curve.PointAtStart, curve.PointAtEnd
    chord = end - start
    length = chord.Length
    if length <= 0.: return dict(degenerate_chord=True)
    chord.Unitize()
    tangents = [curve.TangentAtStart, curve.TangentAtEnd]
    offsets = []
    for tangent in tangents:
        cosine = tangent.X * chord.X + tangent.Y * chord.Y + tangent.Z * chord.Z
        cosine = max(0., min(1., cosine))
        offsets.append(tangent * (length * (2. - cosine * cosine) / 3.))
    definition = dict(degree=3, knots=[0.] * 4 + [1.] * 4,
        control_points=[dict(point=xyz(p), weight=1.) for p in
            [start, start + offsets[0], end - offsets[1], end]])
    if nonrational:
        candidate = geometry.NurbsCurve(3, False, 4, 4)
        host['_set_curve_controls'](candidate, definition['control_points'])
        host['_set_knots'](candidate.Knots, definition['knots'], 'candidate knot')
    else:
        candidate = host['_nurbs_curve_from_definition'](definition)
    try:
        fractions = host['System'].Array[host['System'].Double]([i / 8. for i in range(9)])
        parameters = curve.NormalizedLengthParameters(fractions, 0., fractional_tolerance)
        if parameters is None or len(parameters) != 9:
            raise ValueError('nine-station fitting batch failed')
        points = [curve.PointAt(t) for t in parameters]
        success, parameter = (candidate.ClosestPoint(points[4]) if local_seed is None
            else candidate.LocalClosestPoint(points[4], local_seed))
        if not success: raise ValueError('initial cubic closest query failed')
        point = candidate.PointAt(parameter)
        b1, b2 = 3. * parameter * (1. - parameter) ** 2, 3. * parameter ** 2 * (1. - parameter)
        influence = offsets[0] * b1 - offsets[1] * b2
        residual = points[4] - point
        denominator = influence.SquareLength
        scale = (1. + (influence.X * residual.X + influence.Y * residual.Y + influence.Z * residual.Z) / denominator) if denominator > 0. else None
        if scale is not None and (math.isnan(scale) or math.isinf(scale)): scale = None
        return dict(initial=definition, candidate_is_rational=bool(candidate.IsRational),
            endpoint_tangents=[xyz(t) for t in tangents], chord_direction=xyz(chord),
            endpoint_tangent_dot=sum(a*b for a,b in zip(xyz(tangents[0]),xyz(tangents[1]))),
            candidate_endpoint_tangents=[xyz(candidate.TangentAtStart),xyz(candidate.TangentAtEnd)],
            candidate_endpoint_tangent_dot=sum(a*b for a,b in zip(
                xyz(candidate.TangentAtStart),xyz(candidate.TangentAtEnd))),
            parameters=list(parameters), points=[xyz(p) for p in points],
            midpoint_closest_parameter=float(parameter), midpoint_closest_point=xyz(point),
            midpoint_distance=float(points[4].DistanceTo(point)), scale=scale)
    finally:
        candidate.Dispose()
