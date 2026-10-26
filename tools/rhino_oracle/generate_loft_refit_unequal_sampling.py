"""Distinguish adaptive sampling direction on unequal straight regions."""
import copy
import json
from .capture_family import HERE, capture


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_adaptive_lines.json').read_text())
    template = next(o for o in query['operations'] if o['id'] == 'loft-refit-adaptive-unequal-1e-06')
    operations = []
    for ratio in [.5, 1., 2., 4.]:
        for reverse in [False, True]:
            for limit in [2.5e-7, 5e-7, 1e-6]:
                op = copy.deepcopy(template)
                op['id'] = 'loft-refit-unequal-sampling-{}-{}-{}'.format(ratio, int(reverse), limit)
                op['refit_tolerance'] = limit
                for curve in op['curves']:
                    z = curve['control_points'][0]['point'][2]
                    points = [[0., 0., z], [1., 0., z], [1. + ratio, ratio * 1e-5, z]]
                    if reverse:
                        points.reverse()
                    for control, point in zip(curve['control_points'], points):
                        control['point'] = point
                operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_unequal_sampling', request(),
        ['generate_loft_refit_unequal_sampling.py', 'fixtures/loft_refit_adaptive_lines.json',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitUnequalSampling20261026')
