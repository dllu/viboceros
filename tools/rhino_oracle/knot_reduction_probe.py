"""Public tolerance-bounded knot reduction on a private NURBS duplicate."""


def run(operation, host, iterations, tolerance):
    source = host['_nurbs_curve_from_definition'](operation['curve'])
    minimum = operation.get('minimum_multiplicity', 1)
    maximum = operation.get('maximum_multiplicity', source.Degree + 1)
    limit = host['_finite'](operation.get('absolute_tolerance', tolerance['absolute']),
                            'knot reduction tolerance')
    if type(minimum) is not int or type(maximum) is not int or minimum < 0 or maximum <= minimum or limit < 0.:
        source.Dispose()
        raise ValueError('invalid knot reduction range or tolerance')
    definition = host['_nurbs_curve_definition']
    before = definition(source)

    def compute():
        candidate = source.DuplicateCurve()
        try:
            removed = candidate.Knots.RemoveMultipleKnots(minimum, maximum, limit)
            return dict(removed=int(removed), curve=definition(candidate),
                samples=[host['_xyz'](candidate.PointAt(candidate.Domain.ParameterAt(i / 32.)))
                         for i in range(33)], before=before, after=definition(source))
        finally:
            candidate.Dispose()

    try:
        return host['_measure'](iterations, compute)
    finally:
        source.Dispose()
