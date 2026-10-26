"""Scalar normalized-length queries, including reverse order and the fit clock."""
import math


def inspect(curve):
    result = []
    fractions = [0., 1. / 64., .125, .25, .375, .5, .625, .75, .875, 63. / 64., 1.]
    for accuracy in [1e-8, 1e-6, 1e-4, 1e-3, 1e-2]:
        record = dict(fractional_tolerance=accuracy, length=float(curve.GetLength(accuracy)))
        for key, order in [('forward', fractions), ('reverse', list(reversed(fractions)))]:
            stations = []
            for fraction in order:
                success, parameter = curve.NormalizedLengthParameter(fraction, accuracy)
                point = curve.PointAt(parameter) if success else None
                stations.append(dict(fraction=fraction, success=bool(success),
                    parameter=float(parameter) if success else None,
                    point=[float(point.X), float(point.Y), float(point.Z)] if success else None))
            record[key] = stations
        result.append(record)
    return result


def run(operation, host, iterations, tolerance):
    fractions = operation.get('fractions')
    accuracy = operation.get('fractional_tolerance', 1e-4)
    if (not isinstance(fractions, list) or len(fractions) > 4096 or any(
            type(f) not in (int, float) or math.isnan(f) or math.isinf(f) or not 0. <= f <= 1.
            for f in fractions)):
        raise ValueError('invalid scalar length fractions')
    if type(accuracy) not in (int, float) or not 0. < accuracy < 1.:
        raise ValueError('invalid scalar length tolerance')
    curve = host['_nurbs_curve_from_definition'](operation['curve'])
    try:
        before = host['_nurbs_curve_definition'](curve)
        def compute():
            parameters = []
            for f in fractions:
                success, parameter = curve.NormalizedLengthParameter(f, accuracy)
                if not success: raise ValueError('scalar length inversion failed')
                parameters.append(float(parameter))
            return dict(parameters=parameters, points=[host['_xyz'](curve.PointAt(t)) for t in parameters],
                length=float(curve.GetLength(accuracy)), domain=[curve.Domain.T0, curve.Domain.T1],
                before=before, after=host['_nurbs_curve_definition'](curve))
        return host['_measure'](iterations, compute)
    finally:
        curve.Dispose()
