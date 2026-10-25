"""Inspect public cubic Bezier fitting independently of Loft Refit."""


def cubic_sections(curves, distance, angle, geometry, definition):
    records = []
    for curve in curves:
        beziers = geometry.BezierCurve.CreateCubicBeziers(curve, distance, angle)
        sections = []
        try:
            for bezier in beziers or []:
                if bezier is None:
                    sections.append(None)
                    continue
                nurbs = bezier.ToNurbsCurve()
                try:
                    sections.append(definition(nurbs) if nurbs is not None else None)
                finally:
                    if nurbs is not None:
                        nurbs.Dispose()
        finally:
            for bezier in beziers or []:
                if bezier is not None:
                    bezier.Dispose()
        records.append(sections)
    return records
