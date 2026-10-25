"""Freeze native source positions for independent cubic approximation checks."""
import copy
from .capture_family import capture
from .generate_loft_rebuild import request as loft_request


def request():
    operations = []
    originals = loft_request()['operations']
    for shape in ['line', 'quarter', 'cubic', 'circle', 'polygon']:
        source = next(o for o in originals
                      if o['id'] == 'loft-rebuild-{}-0-normal-0'.format(shape))
        for index, curve in enumerate(source['curves']):
            operations.append(dict(op='curve_native',
                id='cubic-source-{}-{}'.format(shape, index),
                curve=dict(type='nurbs', **copy.deepcopy(curve)), differential_only=True))

    def curve(degree, points, knots, weights=None):
        return dict(type='nurbs', degree=degree, knots=knots,
                    control_points=[dict(point=p, weight=w) for p, w in
                                    zip(points, weights or [1.] * len(points))])

    holdouts = [
        ('hidden-bump', curve(5, [[float(i), y, 0.] for i, y in
            enumerate([0., 6., -13., 13., -6., 0.])], [0.] * 6 + [1.] * 6)),
        ('spatial-rational', curve(3, [[0., 0., 0.], [1., 2., 3.],
            [2., -1., 2.], [4., 3., -1.]], [-3.] * 4 + [9.] * 4,
            [1., .35, 2.2, .8])),
        ('unclamped', curve(2, [[0., 0., 0.], [2., 1., 0.], [3., 0., 1.],
            [4., 2., 0.], [7., 3., 1.]], [-2., -1., 0., 1., 2., 3., 4., 5.])),
        ('nonuniform-kink', curve(3, [[0., 0., 0.], [1., 1., 0.], [2., 0., 1.],
            [3., 1., 1.], [3., 2., 2.], [4., 3., 1.], [5., 3., 2.]],
            [0.] * 4 + [.17] * 3 + [1.] * 4)),
    ]
    shifted = copy.deepcopy(operations[3]['curve'])
    shifted['knots'] = [-17.] * 3 + [29.] * 3
    holdouts.append(('shifted-quarter-domain', shifted))
    for name, source in holdouts:
        operations.append(dict(op='curve_native', id='cubic-source-' + name,
                               curve=source, differential_only=True))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8),
                operations=operations)


if __name__ == '__main__':
    capture('cubic_approximation_sources', request(), [
        'generate_cubic_approximation_sources.py', 'generate_loft_rebuild.py',
        'fixtures/loft.json', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleCubicSource20261025')
