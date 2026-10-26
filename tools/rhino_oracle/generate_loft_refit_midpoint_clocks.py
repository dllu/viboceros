"""Distinguish scalar and batch length inverses near a natural source knot."""
import copy
from .generate_loft_refit_midpoint_snapping import request as original
from .capture_family import capture


def request():
    query = original()
    template = query['operations'][0]
    operations = []
    for offset in [0., 1e-6, 1e-5, 1e-4, 1e-3, 1e-2]:
        for reverse in [False, True]:
            for accuracy in [1e-6, 1e-4, 1e-2]:
                op = copy.deepcopy(template)
                op['id'] = 'loft-refit-midpoint-clock-{}-{}-{}'.format(offset, int(reverse), accuracy)
                op['refit_adjustment_fractional_tolerance'] = accuracy
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
    capture('loft_refit_midpoint_clocks', request(),
        ['generate_loft_refit_midpoint_clocks.py', 'generate_loft_refit_midpoint_snapping.py',
         'fixtures/loft_refit_unequal_sampling.json', 'loft_refit_adjustment_probe.py',
         'loft_scalar_clock_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitMidpointClocks20261026')
