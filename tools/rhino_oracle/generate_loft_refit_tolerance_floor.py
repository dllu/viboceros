"""Locate the fitting floor around twice the documented Bezier minimum."""
import copy
from .capture_family import capture
from .generate_loft_refit_tiny_tolerances import request as tiny


def request():
    query = tiny()
    operations = []
    epsilon = 2. ** -26
    for name in ['bend', 'quarter', 'line']:
        source = next(o for o in query['operations'] if o['id'] == 'loft-refit-tiny-tolerance-' + name + '-0.0')
        for limit in [epsilon * 1.9, epsilon * 2. * (1. - 1e-6), epsilon * 2., epsilon * 2. * (1. + 1e-6), epsilon * 2.1, 5e-8]:
            op = copy.deepcopy(source)
            op['id'] = 'loft-refit-tolerance-floor-{}-{}'.format(name, limit)
            op['refit_tolerance'] = limit
            operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_tolerance_floor', request(),
        ['generate_loft_refit_tolerance_floor.py', 'generate_loft_refit_tiny_tolerances.py',
         'fixtures/loft_refit_adaptive_lines.json', 'fixtures/loft_refit_candidates.json',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitToleranceFloor20261026')
