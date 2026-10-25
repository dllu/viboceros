"""Public compatible-curve and length queries, independent of loft construction."""


def inspect(curve, tolerance, angle_tolerance, geometry, definition):
    record = dict(length_queries=[], compatible=[])
    for accuracy in [1e-8, 1e-3, 1e-2]:
        stations = []
        for i in range(1, 8):
            success, parameter = curve.NormalizedLengthParameter(i / 8., accuracy)
            point = curve.PointAt(parameter) if success else None
            stations.append(dict(fraction=i / 8., success=bool(success),
                parameter=float(parameter) if success else None,
                point=[float(point.X), float(point.Y), float(point.Z)] if success else None))
        record['length_queries'].append(dict(fractional_tolerance=accuracy,
            length=float(curve.GetLength(accuracy)), stations=stations))
    for method in [0, 2]:
        source = curve.DuplicateCurve()
        fitted = []
        try:
            fitted = list(geometry.NurbsCurve.MakeCompatible([source],
                geometry.Point3d.Unset, geometry.Point3d.Unset,
                method, 0, tolerance, angle_tolerance) or [])
            record['compatible'].append(dict(method=method, curves=[definition(c) for c in fitted]))
        finally:
            for c in reversed(fitted):
                c.Dispose()
            source.Dispose()
    return record
