"""Owned public cubic-Bezier fitting and derivative-break diagnostics."""
import math
import re

if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_source
else:
    from geometry_validity_probe import validate as validate_source


def validate(op):
    required = {'op', 'id', 'curve', 'distance', 'angle'}
    if (not isinstance(op, dict) or not required <= set(op)
            or set(op) - required - {'lookup_interval'}
            or op['op'] != 'curve_cubic_structure'
            or not isinstance(op['id'], str)
            or re.match(r'\A[A-Za-z0-9_.-]{1,100}\Z', op['id']) is None):
        raise ValueError('invalid cubic structure recipe')
    for name in ['distance', 'angle']:
        value = op[name]
        if (type(value) not in (int, float) or math.isnan(value) or math.isinf(value)
                or value < 0 or name == 'distance' and value == 0
                or name == 'angle' and value > math.pi):
            raise ValueError('invalid cubic structure tolerance')
    curve = op['curve']
    if (not isinstance(curve, dict) or set(curve) != {'degree', 'knots', 'control_points'}
            or type(curve['degree']) is not int or not 1 <= curve['degree'] <= 16
            or not isinstance(curve['control_points'], list)
            or not curve['degree'] + 1 <= len(curve['control_points']) <= 512
            or not isinstance(curve['knots'], list)
            or len(curve['knots']) != len(curve['control_points']) + curve['degree'] + 1):
        raise ValueError('invalid cubic structure curve dimensions')
    validate_source(dict(op='geometry_validity', id=op['id'], source=dict(type='nurbs', **curve)))
    if 'lookup_interval' in op:
        pair = op['lookup_interval']
        if (not isinstance(pair, list) or len(pair) != 2
                or any(type(x) not in (int, float) or not 0 <= x <= 1 for x in pair)
                or pair[0] >= pair[1]):
            raise ValueError('invalid fitting lookup interval')


def _breaks(curve, geometry):
    result = {}
    for name in ['C0_continuous', 'C1_continuous', 'C2_continuous',
                 'G1_continuous', 'G2_continuous']:
        cursor, end = curve.Domain.T0, curve.Domain.T1
        parameters = []
        for unused in range(513):
            found, parameter = curve.GetNextDiscontinuity(
                getattr(geometry.Continuity, name), cursor, end)
            if not found:
                break
            if not cursor < parameter < end:
                raise ValueError('invalid derivative-break traversal')
            parameters.append(float(parameter))
            cursor = parameter
        else:
            raise ValueError('derivative-break traversal budget exceeded')
        result[name] = parameters
    return result


def run(op, host, iterations, tolerance):
    validate(op)
    if iterations != 1:
        raise ValueError('cubic structure capture requires one iteration')
    geometry = host['Rhino'].Geometry
    source = host['_nurbs_curve_from_definition'](op['curve'])
    curve = None
    try:
        parameters = [float(source.Domain.T0), float(source.Domain.T1)]
        if 'lookup_interval' in op:
            from System import Array, Double
            clock = source.DuplicateCurve()
            try:
                lookup = clock.NormalizedLengthParameters(
                    Array[Double]([i / 8. for i in range(9)]), 0., .01)
                if lookup is None or len(lookup) != 9:
                    raise ValueError('cubic structure lookup failed')
                def parameter(fraction):
                    station = fraction * 8.
                    index = min(int(station), 7)
                    blend = station - index
                    return float((1. - blend) * lookup[index] + blend * lookup[index + 1])
                parameters = [parameter(f) for f in op['lookup_interval']]
            finally:
                clock.Dispose()
            curve = source.Trim(*parameters)
        else:
            curve = source.DuplicateCurve()
        if curve is None or not curve.IsValid:
            raise ValueError('cubic structure input construction failed')
        # Inspect a separate duplicate before fitting, preserving cold caches.
        diagnostic = curve.DuplicateCurve()
        try:
            before = host['_nurbs_curve_definition'](diagnostic)
            breaks = _breaks(diagnostic, geometry)
        finally:
            diagnostic.Dispose()
        def compute():
            beziers = geometry.BezierCurve.CreateCubicBeziers(curve, op['distance'], op['angle'])
            definitions = []
            try:
                if beziers is None or len(beziers) > 4096:
                    raise ValueError('public cubic fitting failed or exceeded its capture budget')
                for bezier in beziers:
                    nurbs = bezier.ToNurbsCurve()
                    try:
                        definitions.append(host['_nurbs_curve_definition'](nurbs))
                    finally:
                        nurbs.Dispose()
            finally:
                for bezier in beziers or []:
                    if bezier is not None:
                        bezier.Dispose()
            if host['_nurbs_curve_definition'](curve) != before:
                raise ValueError('public cubic fitter changed its source definition')
            return dict(source=before, interval_parameters=parameters,
                        discontinuities=breaks, beziers=definitions)
        return host['_measure'](iterations, compute)
    finally:
        if curve is not None:
            curve.Dispose()
        source.Dispose()
