"""Test a predicted one-percent scalar checkpoint boundary during subdivision."""
import copy
from .generate_loft_refit_midpoint_snapping import request as original
from .capture_family import capture


def request():
    query = original()
    template = query['operations'][0]
    operations = []
    for size in [.02, .039, .0392, .03922, .04, .0408, .04082, .05]:
        for offset in [-size, size]:
            for reverse in [False, True]:
                op = copy.deepcopy(template)
                op['id'] = 'loft-refit-midpoint-boundary-{}-{}'.format(offset, int(reverse))
                op.pop('inspect_refit_adjustment')
                op['inspect_scalar_clock'] = True
                ratio = 1. + offset
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
    capture('loft_refit_midpoint_boundary', request(),
        ['generate_loft_refit_midpoint_boundary.py', 'generate_loft_refit_midpoint_snapping.py',
         'fixtures/loft_refit_unequal_sampling.json', 'loft_scalar_clock_probe.py',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitMidpointBoundary20261026')
