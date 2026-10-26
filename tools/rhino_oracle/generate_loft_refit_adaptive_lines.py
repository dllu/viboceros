"""Adaptive linear-region holdouts: scale, joining boundaries and uneven bends."""
import copy
import json
import math
from .capture_family import HERE, capture


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_multispan.json').read_text())
    operations = []
    for name, limit in [('bend-1-0', 1e-6), ('angle-0.05', 1e-6), ('angle-0.25', .001)]:
        original = next(o for o in query['operations'] if
                        o['id'] == 'loft-refit-multispan-{}-plain-{}'.format(name, limit))
        for scale in [.001, 1., 1000.]:
            for factor in [.5, 1., 2.]:
                op = copy.deepcopy(original)
                op['id'] = 'loft-refit-adaptive-{}-{}-{}'.format(name, scale, factor)
                op['refit_tolerance'] = limit * scale * factor
                for curve in op['curves']:
                    for cp in curve['control_points']:
                        cp['point'] = [x * scale for x in cp['point']]
                operations.append(op)
    sources = {}
    for degrees in [.001, .003, .01, .02, .03]:
        angle = math.radians(degrees)
        sources['threshold-' + str(degrees)] = [[0., 0., 0.], [1., 0., 0.],
            [1. + math.cos(angle), math.sin(angle), 0.]]
    sources['unequal'] = [[0., 0., 0.], [1., 0., 0.], [3., 2e-5, 0.]]
    sources['two-bends'] = [[0., 0., 0.], [1., 0., 0.], [2., 1e-5, 0.], [3., 0., 0.]]
    for name, points in sources.items():
        for limit in ([1e-6] if name.startswith('threshold-') else [5e-7, 1e-6]):
            curves = []
            for i in range(3):
                ps = [[x, y, z + 2. * i] for x, y, z in points]
                curves.append(dict(degree=1, knots=[0.] + [float(j) for j in range(len(ps))]
                                   + [float(len(ps) - 1)], control_points=[
                                       dict(point=p, weight=1.) for p in ps]))
            operations.append(dict(op='loft', id='loft-refit-adaptive-{}-{}'.format(name, limit),
                curves=curves, style='straight', closed=False, refit_tolerance=limit,
                inspect_inputs=True, sample_geometry=True, allow_empty=True))
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_adaptive_lines', request(), ['generate_loft_refit_adaptive_lines.py',
        'fixtures/loft_refit_multispan.json', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitAdaptiveLines20261026')
