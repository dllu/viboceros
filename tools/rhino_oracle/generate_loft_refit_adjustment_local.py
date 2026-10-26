"""Compare seeded public closest-point queries with smooth Refit corrections."""
import copy
from .capture_family import capture
from .generate_loft_refit_adjustment_ellipses import request as ellipses


def request():
    query = ellipses()
    operations = []
    for original in query['operations']:
        for seed in [.25, .5, .75]:
            op = copy.deepcopy(original)
            op['id'] += '-local-' + str(seed)
            op['refit_adjustment_local_seed'] = seed
            operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_adjustment_local', request(), ['generate_loft_refit_adjustment_local.py',
        'generate_loft_refit_adjustment_ellipses.py', 'fixtures/loft_refit.json',
        'loft_refit_adjustment_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitAdjustmentLocal20261025')
