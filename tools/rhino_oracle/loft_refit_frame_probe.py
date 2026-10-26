"""Public preparation before fitting, varying only count and coordinate frame."""


def inspect(curve, tolerance, angle, geometry, definition):
    result = []
    variants = [('single', (0., 0., 0.), 1, 0.),
        ('single_z2', (0., 0., 2.), 1, 0.),
        ('single_shift', (7., -3., 2.), 1, 0.),
        ('single_z100', (0., 0., 100.), 1, 0.),
        ('coincident', (0., 0., 0.), 2, 0.),
        ('translated', (0., 0., 0.), 2, 2.),
        ('translated_shift', (7., -3., 2.), 2, 2.)]
    for name, offset, count, step in variants:
        sources = []
        try:
            for i in range(count):
                source = curve.DuplicateCurve()
                sources.append(source)
                if not source.Transform(geometry.Transform.Translation(
                        offset[0], offset[1], offset[2] + step * i)):
                    raise ValueError('could not transform a private profile')
            record = dict(variant=name, methods=[])
            for method in [0, 2]:
                fitted = []
                try:
                    fitted = list(geometry.NurbsCurve.MakeCompatible(sources,
                        geometry.Point3d.Unset, geometry.Point3d.Unset,
                        method, 0, tolerance, angle) or [])
                    for c in fitted:
                        if not c.Transform(geometry.Transform.Translation(*[-x for x in offset])):
                            raise ValueError('could not restore the private result frame')
                    record['methods'].append(dict(method=method, curves=[definition(c) for c in fitted]))
                finally:
                    for c in reversed(fitted): c.Dispose()
            result.append(record)
        finally:
            for c in reversed(sources): c.Dispose()
    return result
