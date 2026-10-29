"""Compare public tangent rounding for polynomial and unit-weight storage."""
import copy
from .capture_family import capture
from .generate_loft_refit_endpoint_precision import request as endpoints

FAMILY = 'loft_refit_candidate_precision'


def request():
    query = endpoints()
    operations = []
    for original in query['operations']:
        for nonrational in [False,True]:
            op = copy.deepcopy(original)
            op['id'] = op['id'].replace('endpoint-precision-', 'candidate-precision-')
            op['id'] += '-polynomial' if nonrational else '-unit-weights'
            op['refit_adjustment_nonrational'] = nonrational
            operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture(FAMILY,request(),['generate_loft_refit_candidate_precision.py',
        'generate_loft_refit_endpoint_precision.py','fixtures/sweep1_refit_opposed_cosines.json',
        'loft_refit_adjustment_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleCandidatePrecision20261029',timeout=300)
