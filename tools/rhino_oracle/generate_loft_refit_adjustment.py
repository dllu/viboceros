"""Initial and adjusted smooth proposals measured through public Rhino APIs."""
import copy
from .capture_family import capture
from .generate_loft_refit_coarse import request as coarse


def request():
    query = coarse()
    originals = [o for o in query['operations'] if o['refit_tolerance'] == .1
        and o['id'] != 'loft-refit-coarse-circle-0.1']
    operations = []
    for original in originals:
        name = original['id'][len('loft-refit-coarse-'):-len('-0.1')]
        for tolerance in [.03, .1]:
            op = copy.deepcopy(original)
            op['id'] = 'loft-refit-adjustment-{}-{}'.format(name, tolerance)
            op['refit_tolerance'] = tolerance
            op['inspect_refit_adjustment'] = True
            operations.append(op)
        if name in ['quarter', 'quadratic_2']:
            for variant in ['domain', 'frame']:
                op = copy.deepcopy(operations[-1])
                op['id'] += '-' + variant
                for c in op['curves']:
                    if variant == 'domain': c['knots'] = [-3. + 10. * t for t in c['knots']]
                    for cp in c['control_points']:
                        x, y, z = cp['point']
                        if variant == 'frame': cp['point'] = [7. + z, -3. + x, 5. + y]
                operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_adjustment', request(), ['generate_loft_refit_adjustment.py',
        'generate_loft_refit_coarse.py', 'generate_loft_refit_policy.py', 'generate_loft_refit.py',
        'generate_loft_rebuild.py', 'fixtures/loft.json', 'loft_refit_adjustment_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitAdjustment20261025')
