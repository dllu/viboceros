"""Separate local fitting acceptance from parent subdivision history."""
import copy
import json
from .capture_family import HERE, capture


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_adaptive_lines.json').read_text())
    template = next(o for o in query['operations'] if o['id'] == 'loft-refit-adaptive-unequal-1e-06')
    operations = []
    for length, corner, limit in [(.75, .25, 1e-6), (.375, .25, 5e-7)]:
        for reverse in [False, True]:
            for offset in [0., .75, 1000.]:
                op = copy.deepcopy(template)
                op['id'] = 'loft-refit-interval-sampling-{}-{}-{}'.format(length, int(reverse), offset)
                op['refit_tolerance'] = limit
                for curve in op['curves']:
                    z = curve['control_points'][0]['point'][2]
                    points = [[offset, 0., z], [offset + corner, 0., z],
                              [offset + length, (length - corner) * 1e-5, z]]
                    if reverse:
                        points.reverse()
                    for control, point in zip(curve['control_points'], points):
                        control['point'] = point
                operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_interval_sampling', request(),
        ['generate_loft_refit_interval_sampling.py', 'fixtures/loft_refit_adaptive_lines.json',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitIntervalSampling20261026')
