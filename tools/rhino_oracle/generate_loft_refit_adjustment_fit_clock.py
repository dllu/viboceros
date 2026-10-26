"""Measure the loose public NURBS batch clock and rational candidate variants."""
import copy
from .capture_family import capture
from .generate_loft_refit_adjustment import request as original


def request():
    query = original()
    operations = []
    for original_op in query['operations']:
        for nonrational in [False, True]:
            op = copy.deepcopy(original_op)
            op['id'] = op['id'].replace('loft-refit-adjustment-', 'loft-refit-fit-clock-')
            op['id'] += '-polynomial' if nonrational else '-rational'
            op['refit_adjustment_fractional_tolerance'] = 1e-2
            op['refit_adjustment_nonrational'] = nonrational
            operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_adjustment_fit_clock', request(), ['generate_loft_refit_adjustment_fit_clock.py',
        'generate_loft_refit_adjustment.py', 'generate_loft_refit_coarse.py',
        'generate_loft_refit_policy.py', 'generate_loft_refit.py', 'generate_loft_rebuild.py',
        'fixtures/loft.json', 'loft_refit_adjustment_probe.py', 'rhino_worker.py', 'client.py',
        'structural_sdk.py'], 'VibocerosOracleLoftRefitAdjustmentFitClock20261025')
