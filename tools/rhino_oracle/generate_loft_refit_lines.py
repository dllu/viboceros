"""Public Loft Refit references for straight profiles with varied NURBS clocks."""
import copy
from .capture_family import capture


def request():
    shapes = [
        ('line', 1, [0., 1.], None, None),
        ('quadratic', 2, [0., .25, 1.], None, None),
        ('uneven_cubic', 3, [0., .0625, .25, 1.], None, None),
        ('rational_cubic', 3, [0., .125, .5, 1.], [1., 2., .5, 1.], None),
        ('quintic', 5, [0., .0625, .125, .25, .5, 1.], None, None),
        ('multi_cubic', 3, [0., .125, .5, .75, 1.], None, [0.] * 4 + [.3] + [1.] * 4),
        ('polyline', 1, [0., .25, 1.], None, [0., 0., .3, 1., 1.]),
        ('stationary_ends', 3, [0., 0., 1., 1.], None, None),
    ]
    operations = []
    for name, degree, fractions, weights, knots in shapes:
        for style in ['normal', 'loose', 'tight', 'straight', 'uniform']:
            for tolerance in [.001, 1e-6]:
                curves = []
                for index in range(3):
                    start = [float(index), -2., float(2 * index)]
                    direction = [4. + 2. * index, 2. + index, 0.]
                    points = [dict(point=[a + t * d for a, d in zip(start, direction)],
                                   weight=(weights or [1.] * len(fractions))[j])
                              for j, t in enumerate(fractions)]
                    curves.append(dict(degree=degree, control_points=points,
                                       knots=knots or [0.] * (degree + 1) + [1.] * (degree + 1)))
                operations.append(dict(op='loft', id='loft-refit-lines-{}-{}-{}'.format(name, style, tolerance),
                    curves=curves, style=style, closed=False, refit_tolerance=tolerance,
                    inspect_inputs=True, sample_geometry=True, allow_empty=True))
    # Parameter domains and uniform weight gauges must not define line speed.
    base = next(o for o in operations if o['id'] == 'loft-refit-lines-uneven_cubic-normal-0.001')
    for label, domain, gauge in [('shifted', [-7., 13.], 1.), ('wide', [0., 100.], 1.),
                                  ('large_weight', [0., 1.], 1e100)]:
        op = copy.deepcopy(base)
        op['id'] = 'loft-refit-lines-' + label
        for curve in op['curves']:
            curve['knots'] = [domain[0] + k * (domain[1] - domain[0]) for k in curve['knots']]
            for cp in curve['control_points']:
                cp['weight'] *= gauge
        operations.append(op)
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_refit_lines', request(), ['generate_loft_refit_lines.py', 'rhino_worker.py',
        'client.py', 'structural_sdk.py'], 'VibocerosOracleLoftRefitLines20261025')
