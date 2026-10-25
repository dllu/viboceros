"""Capture corner preservation and section matching in Loft Refit."""
import copy
from .capture_family import capture


def polyline(points, weights=None, knots=None, degree=1):
    return dict(degree=degree, knots=knots or [0., 0.] + list(range(1, len(points) - 1))
        + [float(len(points) - 1)] * 2, control_points=[dict(point=p, weight=w)
        for p, w in zip(points, weights or [1.] * len(points))])


def request():
    corner = polyline([[0., 0., 0.], [2., 0., 0.], [2., 1., 0.]])
    rectangle = polyline([[0., 0., 0.], [2., 0., 0.], [2., 1., 0.], [0., 1., 0.], [0., 0., 0.]])
    space = polyline([[0., 0., 0.], [1., 0., 1.], [2., 1., 0.], [3., 1., 2.]])
    subdivided = polyline([[0., 0., 0.], [.5, 0., 0.], [2., 0., 0.], [2., 1., 0.]],
        knots=[-3., -3., -2., 4., 7., 7.])
    rational = polyline([[0., 0., 0.], [.5, 0., 0.], [2., 0., 0.], [2., .25, 0.], [2., 1., 0.]],
        [1., 2., 1., 4., 1.], [0., 0., 0., 1., 1., 2., 2., 2.], degree=2)
    unclamped = polyline([[0., 0., 0.], [1., 0., 0.], [2., 0., 0.], [2., 1., 0.], [2., 2., 0.]],
        [1., 2., 1., 2., 1.], [-1., -1., 0., 1., 1., 2., 3., 3.], degree=2)
    backtrack = polyline([[0., 0., 0.], [2., 0., 0.], [1., 0., 0.], [1., 1., 0.]])
    small_kink = polyline([[0., 0., 0.], [1., 0., 0.], [2., .00001, 0.]])
    constant_span = polyline([[0., 0., 0.], [1., 0., 0.], [1., 0., 0.], [1., 1., 0.]])
    contexts = dict(corner=[corner] * 3, rectangle=[rectangle] * 3, space=[space] * 3,
        subdivided=[subdivided] * 3, rational=[rational] * 3, unclamped=[unclamped] * 3,
        mixed_counts=[corner, rectangle, space], backtrack=[backtrack] * 3,
        small_kink=[small_kink] * 3, constant_span=[constant_span] * 3)
    operations = []
    for name, templates in contexts.items():
        curves = [copy.deepcopy(c) for c in templates]
        for i, c in enumerate(curves):
            for cp in c['control_points']:
                x, y, z = cp['point']
                cp['point'] = [(1. + .25 * i) * x + .125 * i, (1. + .125 * i) * y, z + 3. * i]
        for style in ['normal', 'loose', 'tight', 'straight', 'uniform']:
            for tolerance in [.001, 1e-6]:
                operations.append(dict(op='loft', id='loft-refit-polyline-{}-{}-{}'.format(name, style, tolerance),
                    curves=copy.deepcopy(curves), style=style, closed=False, refit_tolerance=tolerance,
                    inspect_inputs=True, sample_geometry=True, allow_empty=True))
    for original in [o for o in operations if '-rectangle-' in o['id']]:
        op = copy.deepcopy(original); op['id'] += '-closed'; op['closed'] = True; operations.append(op)
    return dict(protocol_version=1, iterations=1, tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8),
        operations=operations)


if __name__ == '__main__':
    capture('loft_refit_polylines', request(), ['generate_loft_refit_polylines.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'], 'VibocerosOracleLoftRefitPolylines20261025')
