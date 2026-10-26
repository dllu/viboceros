"""Separate sampling and closest-point errors in noncircular rational profiles."""
import copy
import json
from .capture_family import HERE, capture


def request():
    old = json.loads((HERE / 'fixtures/loft_refit.json').read_text())
    original = next(o for o in old['operations'] if o['id'] == 'loft-refit-quarter-0.1-normal')
    operations = []
    for index, source in enumerate(original['curves']):
        for tolerance in [.03, .1]:
            for nonrational in [False, True]:
                other = copy.deepcopy(source)
                for cp in other['control_points']: cp['point'][2] += 5.
                operations.append(dict(op='loft', id='loft-refit-ellipse-{}-{}-{}'.format(index, tolerance, nonrational),
                    curves=[copy.deepcopy(source), other], style='straight', closed=False,
                    refit_tolerance=tolerance, sample_geometry=True, inspect_inputs=True,
                    inspect_refit_adjustment=True, refit_adjustment_fractional_tolerance=1e-2,
                    refit_adjustment_nonrational=nonrational, allow_empty=True))
        op = copy.deepcopy(original)
        op['id'] = 'loft-refit-ellipse-original-' + str(index)
        op['curves'] = copy.deepcopy(original['curves'][index:] + original['curves'][:index])
        op.update(inspect_refit_adjustment=True, refit_adjustment_fractional_tolerance=1e-2)
        operations.append(op)
    old['operations'] = operations
    return old


if __name__ == '__main__':
    capture('loft_refit_adjustment_ellipses', request(), ['generate_loft_refit_adjustment_ellipses.py',
        'fixtures/loft_refit.json', 'loft_refit_adjustment_probe.py', 'rhino_worker.py',
        'client.py', 'structural_sdk.py'], 'VibocerosOracleLoftRefitAdjustmentEllipses20261025')
