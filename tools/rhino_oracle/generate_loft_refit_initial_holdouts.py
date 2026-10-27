"""Curved multispan holdouts near initial-fit tolerance transitions."""
import copy
import json
from .capture_family import HERE, capture


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_multispan.json').read_text())
    operations = []
    for name in ['smooth-split', 'curvature-jump']:
        original = next(op for op in query['operations']
                        if op['id'] == 'loft-refit-multispan-' + name + '-plain-1e-06')
        for unequal in [False, True]:
            for tolerance in [2e-6, 3e-6, 4e-6, 6e-6, 8e-6, 1e-5]:
                op = copy.deepcopy(original)
                op['id'] = 'loft-refit-initial-holdout-{}-{}-{}'.format(name, int(unequal), tolerance)
                op['refit_tolerance'] = tolerance
                if unequal:
                    for curve in op['curves']:
                        for control in curve['control_points']:
                            x = control['point'][0]
                            control['point'][0] = x * .25 if x <= 1. else .25 + .5 * (x - 1.)
                operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_initial_holdouts', request(), [
        'generate_loft_refit_initial_holdouts.py', 'fixtures/loft_refit_multispan.json',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitInitialHoldouts20261027')
