"""Check public fitting on raw/simplified single and paired Loft inputs."""
from loft_refit_probe import cubic_sections


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
