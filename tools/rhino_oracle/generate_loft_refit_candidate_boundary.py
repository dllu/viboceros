"""Test the initial smooth proposal's half-tolerance acceptance boundary."""
import copy
import math
from .capture_family import capture
from .generate_loft_refit_coarse import request as coarse


def request():
    query = coarse()
    operations = []
    # For these symmetric profiles the maximum initial-cubic image error is
    # attained at the midpoint. Test both sides of twice that distance.
    thresholds = [('quarter', 2. * (math.sqrt(2.) + .75 - 2.)),
        ('quadratic_1', 2. * (1.5 - .75 * 1.877446817993083))]
    for name, boundary in thresholds:
        original = next(o for o in query['operations']
            if o['id'] == 'loft-refit-coarse-' + name + '-1.0')
        for variant in ['original', 'scaled', 'frame', 'domain']:
            for side, factor in [('below', 1. - 1e-4), ('above', 1. + 1e-4)]:
                op = copy.deepcopy(original)
                op['id'] = 'loft-refit-candidate-boundary-{}-{}-{}'.format(name, variant, side)
                op['refit_tolerance'] = boundary * factor * (2. if variant == 'scaled' else 1.)
                for c in op['curves']:
                    if variant == 'domain': c['knots'] = [-3. + 10. * t for t in c['knots']]
                    for cp in c['control_points']:
                        x, y, z = cp['point']
                        if variant == 'scaled': cp['point'] = [2.*x, 2.*y, 2.*z]
                        if variant == 'frame': cp['point'] = [7.+z, -3.+x, 5.+y]
                operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_candidate_boundary', request(), ['generate_loft_refit_candidate_boundary.py',
        'generate_loft_refit_coarse.py', 'generate_loft_refit_policy.py', 'generate_loft_refit.py',
        'generate_loft_rebuild.py', 'fixtures/loft.json', 'rhino_worker.py', 'client.py',
        'structural_sdk.py'], 'VibocerosOracleLoftRefitCandidateBoundary20261025')
