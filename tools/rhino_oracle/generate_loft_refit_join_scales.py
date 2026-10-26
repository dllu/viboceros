"""Separate raw-piece sampling from the scale-sensitive composite C1 branch."""
import copy
import json
from .capture_family import HERE, capture


def request():
    original = json.loads((HERE / 'fixtures/loft_refit_adaptive_lines.json').read_text())
    operations = []
    for name in ['angle-0.05', 'angle-0.25']:
        source = next(o for o in original['operations'] if o['id'] == 'loft-refit-adaptive-' + name + '-1.0-1.0')
        for scale in [1., 2., 4., 8., 16., 32., 64., 128., 256., 512.]:
            op = copy.deepcopy(source)
            op['id'] = 'loft-refit-join-scales-{}-{}'.format(name, scale)
            op['refit_tolerance'] *= scale
            for curve in op['curves']:
                for cp in curve['control_points']:
                    cp['point'] = [scale * x for x in cp['point']]
            operations.append(op)
    original['operations'] = operations
    return original


if __name__ == '__main__':
    capture('loft_refit_join_scales', request(),
        ['generate_loft_refit_join_scales.py', 'fixtures/loft_refit_adaptive_lines.json',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitJoinScales20261026')
