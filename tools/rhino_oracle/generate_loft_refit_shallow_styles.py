"""Explicit shallow corners through styles, point ends and a closed radial loft."""
import copy
import json
import math
from .capture_family import HERE, capture


def request():
    base = json.loads((HERE / 'fixtures/loft_refit_shallow_angles.json').read_text())
    holdout = json.loads((HERE / 'fixtures/loft_refit_shallow_holdouts.json').read_text())
    quarter = next(o for o in base['operations'] if o['id'] == 'loft-refit-shallow-angle-0.25-0.0')
    unequal = next(o for o in holdout['operations'] if o['id'] == 'loft-refit-shallow-holdout-unequal')
    operations = []
    for name, original in [('quarter', quarter), ('unequal', unequal)]:
        for style in ['normal', 'loose', 'tight', 'straight', 'uniform']:
            op = copy.deepcopy(original)
            op['id'] = 'loft-refit-shallow-styles-{}-{}'.format(name, style)
            op['style'] = style
            operations.append(op)
    for name in ['start', 'end', 'both']:
        op = copy.deepcopy(quarter)
        op['id'] = 'loft-refit-shallow-styles-point-' + name
        if name in ['start', 'both']:
            op['start_point'] = [0., 0., -2.]
        if name in ['end', 'both']:
            op['end_point'] = [0., 0., 6.]
        operations.append(op)
    op = copy.deepcopy(quarter)
    op['id'] = 'loft-refit-shallow-styles-closed-radial'
    op['closed'] = True
    for i, curve in enumerate(op['curves']):
        angle = i * 2. * math.pi / 3.
        for cp in curve['control_points']:
            x, y, _ = cp['point']
            cp['point'] = [(2. + x) * math.cos(angle), (2. + x) * math.sin(angle), y]
    operations.append(op)
    base['operations'] = operations
    return base


if __name__ == '__main__':
    capture('loft_refit_shallow_styles', request(),
        ['generate_loft_refit_shallow_styles.py', 'fixtures/loft_refit_shallow_angles.json',
         'fixtures/loft_refit_shallow_holdouts.json', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitShallowStyles20261026')
