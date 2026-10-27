"""Measure paired fitting when length stations miss narrow source features."""
import copy
import math
from .capture_family import capture


def request():
    shapes = [
        [[0., 0.], [.049, 0.], [.05, 1e-6], [.051, 0.], [1., 0.]],
        [[0., 0.], [.049, 0.], [.05, 1e-6], [.051, 0.],
         [.949, 0.], [.95, -1e-6], [.951, 0.], [1., 0.]],
    ]
    operations = []
    for shape, original in enumerate(shapes):
        for scale in [1., 1024.]:
            for reverse in [False, True]:
                points = [p[:] for p in reversed(original)] if reverse else copy.deepcopy(original)
                for clock in ['index', 'length']:
                    positions = [float(i) for i in range(len(points))]
                    if clock == 'length':
                        positions = [0.]
                        for a, b in zip(points, points[1:]):
                            positions.append(positions[-1] + math.dist(a, b))
                    positions = [t / positions[-1] for t in positions]
                    curves = [dict(degree=1, knots=[0.] + positions + [1.],
                        control_points=[dict(point=[scale*p[0], scale*p[1], scale*z], weight=1.)
                                        for p in points]) for z in [0., 2., 4.]]
                    operations.append(dict(op='loft',
                        id='loft-refit-hidden-{}-{}-{}-{}'.format(shape, scale, int(reverse), clock),
                        curves=curves, style='straight', closed=False, refit_tolerance=scale*1e-7,
                        inspect_inputs=True, inspect_refit_sources=True, sample_geometry=True, allow_empty=True))
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_refit_hidden_features', request(),
        ['generate_loft_refit_hidden_features.py', 'loft_refit_source_probe.py', 'loft_refit_probe.py',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitHiddenFeatures20261026')
