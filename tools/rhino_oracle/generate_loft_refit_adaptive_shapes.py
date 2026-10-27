"""Independent smooth, rational, spatial and higher-degree adaptive holdouts."""
from .capture_family import capture


def request():
    x = [0., .05, .2, .35, .55, .7, .9, 1.]
    y = [0., 0., .001, -.002, .003, -.001, 0., 0.]
    knots = [0.] * 4 + [.2, .4, .6, .8] + [1.] * 4
    sources = [
        ('smooth', 3, knots, list(zip(x, y, [0.] * 8)), [1.] * 8),
        ('rational', 3, knots, list(zip(x, y, [0.] * 8)),
         [1., 2., .5, 3., 1.5, 1., .75, 1.]),
        ('spatial', 3, knots,
         list(zip(x, y, [0., 0., -.001, .002, .001, -.002, 0., 0.])), [1.] * 8),
        ('quintic', 5, [0.] * 6 + [1.] * 6,
         list(zip([0., .1, .2, .6, .9, 1.],
                  [0., 0., .001, -.002, .001, 0.], [0.] * 6)), [1.] * 6),
    ]
    operations = []
    for name, degree, source_knots, points, weights in sources:
        for tolerance in [1e-5, 3e-5, 1e-4]:
            curves = []
            for height in [0., 2., 4.]:
                curves.append(dict(degree=degree, knots=source_knots,
                    control_points=[dict(point=[x, y, z + height], weight=w)
                                    for (x, y, z), w in zip(points, weights)]))
            operations.append(dict(id='loft-refit-adaptive-shapes-{}-{}'.format(name, tolerance),
                op='loft', style='straight', closed=False, curves=curves,
                refit_tolerance=tolerance))
    return dict(protocol_version=1, iterations=1, operations=operations)


if __name__ == '__main__':
    capture('loft_refit_adaptive_shapes', request(), [
        'generate_loft_refit_adaptive_shapes.py', 'rhino_worker.py', 'client.py',
        'structural_sdk.py'], 'VibocerosOracleLoftRefitAdaptiveShapes20261027')
