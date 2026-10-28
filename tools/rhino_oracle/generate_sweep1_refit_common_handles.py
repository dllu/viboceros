"""Controlled cubic profiles with one known, common endpoint-handle scale."""
import copy
import math

from .capture_family import capture
from .generate_sweep1_refit import request as base_request


def request():
    root = base_request()
    template = next(op for op in root['operations'] if op['id'] == 'open-refit-0.01')
    operations = []
    for scale in [.9, .97, .99, .995, 1., 1.01]:
        curve = dict(type='nurbs', degree=3, knots=[0.] * 4 + [1.] * 4,
                     control_points=[dict(point=p, weight=1.) for p in [
                         [0., 0., 0.],
                         [scale * 1.8 / math.sqrt(5.), scale * 3.6 / math.sqrt(5.), 0.],
                         [3. - scale * 1.5 / math.sqrt(2.), scale * 1.5 / math.sqrt(2.), 0.],
                         [3., 0., 0.]]])
        for tolerance in [.001, .01]:
            for count in ([1, 2] if scale in [.9, .99] else [1]):
                op = copy.deepcopy(template)
                sections = [copy.deepcopy(curve)]
                if count == 2:
                    second = copy.deepcopy(curve)
                    for point in second['control_points']:
                        point['point'][2] = 5.
                    sections.append(second)
                op.update(id='common-{}-{}-{}'.format(scale, count, tolerance),
                          sections=sections, parameters=[0.] if count == 1 else [0., 5.],
                          refit_tolerance=tolerance)
                operations.append(op)
    root['operations'] = operations
    return root


if __name__ == '__main__':
    capture('sweep1_refit_common_handles', request(), [
        'generate_sweep1_refit_common_handles.py', 'generate_sweep1_refit.py',
        'generate_sweep1_rebuild.py', 'sweep1_probe.py', 'rhino_worker.py',
        'client.py', 'structural_sdk.py'],
        'VibocerosOracleSweepRefitCommonHandles20261028', timeout=300)
