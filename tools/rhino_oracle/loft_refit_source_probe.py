"""Check public fitting on raw/simplified single and paired Loft inputs."""
from loft_refit_probe import cubic_sections


def source_structure(curve, geometry, definition):
    """Inspect an owned source without affecting the fitting operands."""
    source = curve.DuplicateCurve()
    nurbs = None
    try:
        domain = source.Domain
        discontinuities = {}
        for name in ['C1_continuous', 'G1_continuous', 'G2_continuous']:
            cursor, cuts = domain.T0, []
            for unused in range(513):
                found, parameter = source.GetNextDiscontinuity(
                    getattr(geometry.Continuity, name), cursor, domain.T1)
                if not found:
                    break
                if not cursor < parameter < domain.T1:
                    raise ValueError('invalid source discontinuity traversal')
                cuts.append(float(parameter))
                cursor = parameter
            else:
                raise ValueError('source discontinuity budget exceeded')
            discontinuities[name] = cuts
        spans = []
        for i in range(source.SpanCount):
            span = source.SpanDomain(i)
            first = source.DerivativeAt(span.T0, 1, geometry.CurveEvaluationSide.Above)[1]
            last = source.DerivativeAt(span.T1, 1, geometry.CurveEvaluationSide.Below)[1]
            spans.append(dict(domain=[float(span.T0), float(span.T1)],
                derivatives=[[float(v.X), float(v.Y), float(v.Z)] for v in [first, last]]))
        nurbs = source.ToNurbsCurve()
        return dict(nurbs=definition(nurbs), spans=spans, discontinuities=discontinuities)
    finally:
        if nurbs is not None:
            nurbs.Dispose()
        source.Dispose()


def numerical_clocks(curve):
    # Inspect an owned duplicate so diagnostics cannot affect fitting caches.
    from System import Array, Double
    source = curve.DuplicateCurve()
    try:
        fractions = Array[Double]([i / 8. for i in range(9)])
        rows = []
        for accuracy in [1e-4, 1e-3, 5e-3, 1e-2]:
            scalars = []
            for fraction in [.25, .5, .75]:
                success, parameter = source.NormalizedLengthParameter(fraction, accuracy)
                if not success:
                    raise ValueError('source scalar clock failed')
                point = source.PointAt(parameter)
                scalars.append(dict(fraction=fraction, parameter=float(parameter),
                    point=[float(point.X), float(point.Y), float(point.Z)]))
            batch = source.NormalizedLengthParameters(fractions, 0., accuracy)
            if batch is None or len(batch) != 9:
                raise ValueError('source batch clock failed')
            rows.append(dict(fractional_tolerance=accuracy,
                length=float(source.GetLength(accuracy)), scalars=scalars,
                batch_parameters=[float(t) for t in batch]))
        return rows
    finally:
        source.Dispose()

def inspect(curves, tolerance, angle, absolute, geometry, definition):
    records = []
    for simplify in [False, True]:
        sources = []
        try:
            for original in curves:
                source = original.Simplify(geometry.CurveSimplifyOptions.All, absolute, angle) if simplify else None
                sources.append(source if source is not None else original.DuplicateCurve())
            for count in [1, 2]:
                fitted = []
                try:
                    fitted = list(geometry.NurbsCurve.MakeCompatible(sources[:count],
                        geometry.Point3d.Unset, geometry.Point3d.Unset,
                        2, 0, tolerance, angle) or [])
                    records.append(dict(simplified=simplify, source_count=count,
                        source_types=[s.GetType().Name for s in sources[:count]],
                        source_structure=source_structure(sources[0], geometry, definition),
                        numerical_clocks=numerical_clocks(sources[0]),
                        curves=[definition(c) for c in fitted]))
                finally:
                    for c in reversed(fitted):
                        c.Dispose()
            if simplify:
                records.append(dict(simplified_beziers=cubic_sections(sources[:1],
                    tolerance, angle, geometry, definition)))
        finally:
            for c in reversed(sources):
                c.Dispose()
    return records
