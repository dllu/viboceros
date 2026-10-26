"""Locate how paired fitting treats a natural knot near an arc midpoint."""
import copy
import json
from .capture_family import HERE, capture


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_unequal_sampling.json').read_text())
    template = query['operations'][0]
    operations = []
    for offset in [-1e-4, -1e-5, -1e-6, -1e-7, -1e-8, -1e-9, -1e-10,
                   0., 1e-10, 1e-9, 1e-8, 1e-7, 1e-6, 1e-5, 1e-4]:
        for reverse in [False, True]:
            op = copy.deepcopy(template)
            op['id'] = 'loft-refit-midpoint-snapping-{}-{}'.format(offset, int(reverse))
            op['refit_tolerance'] = 1e-6
            op['inspect_refit_adjustment'] = True
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
    capture('loft_refit_midpoint_snapping', request(),
        ['generate_loft_refit_midpoint_snapping.py', 'fixtures/loft_refit_unequal_sampling.json',
         'loft_refit_adjustment_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitMidpointSnapping20261026')
