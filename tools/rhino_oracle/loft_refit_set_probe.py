"""Compare public Refit preparation across profile count and companion shape."""
import math


def inspect(curve, tolerance, angle, geometry, definition):
    result = []
    variants = [('single', 1, 1., False), ('single_sqrt2', 1, math.sqrt(2.), False),
        ('single_twice', 1, 2., False), ('single_fourfold', 1, 4., False),
        ('paired', 2, 1., False), ('triple', 3, 1., False), ('quadruple', 4, 1., False),
        ('larger_companion', 2, 1., True), ('line_companion', 2, 1., False)]
    for name, count, scale, larger in variants:
        sources, fitted = [], []
        try:
            for index in range(count):
                if name == 'line_companion' and index == 1:
                    source = geometry.LineCurve(geometry.Point3d(0., 0., 2.), geometry.Point3d(4., 1., 2.))
                else:
                    source = curve.DuplicateCurve()
                    sources.append(source)
                    factor = 2. if larger and index else scale
                    if factor != 1. and not source.Transform(geometry.Transform.Scale(geometry.Point3d.Origin, factor)):
                        raise ValueError('could not scale a private refit profile')
                    if index and not source.Transform(geometry.Transform.Translation(0., 0., 2. * index)):
                        raise ValueError('could not translate a private refit profile')
                    continue
                sources.append(source)
            fitted = list(geometry.NurbsCurve.MakeCompatible(sources,
                geometry.Point3d.Unset, geometry.Point3d.Unset, 2, 0, tolerance, angle) or [])
            if scale != 1.:
                for c in fitted:
                    if not c.Transform(geometry.Transform.Scale(geometry.Point3d.Origin, 1. / scale)):
                        raise ValueError('could not restore the scale of a refit result')
            result.append(dict(variant=name, count=count, input_scale=scale,
                curves=[definition(c) for c in fitted]))
        finally:
            for c in reversed(fitted):
                c.Dispose()
            for c in reversed(sources):
                c.Dispose()
    return result
