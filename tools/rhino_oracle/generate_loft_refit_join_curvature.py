"""Refine physical-curvature boundaries independently of positional fit ratio."""
import copy
import json
import math
from .capture_family import HERE, capture


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_join_scales.json').read_text())
    original = next(o for o in query['operations'] if o['id'] == 'loft-refit-join-scales-angle-0.25-1.0')
    operations = []
    for scale, angle in [(s, None) for s in [1.70, 1.74, 1.75, 1.80, 3.45, 3.49, 3.50, 3.55]] + [(s, math.radians(10.)) for s in [1., 2., 4.]]:
        op = copy.deepcopy(original)
        op['id'] = 'loft-refit-join-curvature-{}-{}'.format(scale, angle)
        op['refit_tolerance'] *= scale
        for c in op['curves']:
            for p in c['control_points']:
                p['point'] = [scale * x for x in p['point']]
        if angle is not None:
            op['refit_angle_tolerance'] = angle
        operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_join_curvature', request(),
        ['generate_loft_refit_join_curvature.py', 'fixtures/loft_refit_join_scales.json',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitJoinCurvature20261026')
