"""Coarse smooth fits across profiles, styles, end points and parameter gauges."""
import copy
from .capture_family import capture
from .generate_loft_refit_coarse import request as coarse


def request():
    query = coarse()
    source_ops = [o for o in query['operations'] if o['refit_tolerance'] == 1.
        and any(o['id'] == 'loft-refit-coarse-' + n + '-1.0'
            for n in ['quarter', 'quadratic', 'polynomial_cubic', 'polynomial_cubic_2'])]
    operations = []
    for original in source_ops:
        name = original['id'][len('loft-refit-coarse-'):-len('-1.0')]
        def add(suffix, **extra):
            op = copy.deepcopy(original)
            op['id'] = 'loft-refit-candidate-' + name + '-' + suffix
            op.update(extra)
            operations.append(op)
            return op
        for style in ['normal', 'loose', 'tight', 'straight', 'uniform']:
            op = add(style, style=style)
            third = copy.deepcopy(original['curves'][0])
            for cp in third['control_points']:
                cp['point'][2] += 5.
            op['curves'].append(third)
        op = add('closed', closed=True)
        third = copy.deepcopy(original['curves'][0])
        for cp in third['control_points']:
            cp['point'][2] += 5.
        op['curves'].append(third)
        op = add('different-companion')
        other = copy.deepcopy(next(o for o in source_ops
            if o['id'] == 'loft-refit-coarse-polynomial_cubic-1.0')['curves'][1])
        op['curves'][1] = other
        op = add('domain-weight-gauge')
        for c in op['curves']:
            c['knots'] = [-3. + 10. * t for t in c['knots']]
            for cp in c['control_points']:
                cp['weight'] *= -7.
        op = add('frame')
        for c in op['curves']:
            for cp in c['control_points']:
                x, y, z = cp['point']
                cp['point'] = [7. + z, -3. + x, 5. + y]
        for end in ['start', 'end', 'both']:
            extra = {}
            if end in ['start', 'both']: extra['start_point'] = [0., 0., -2.]
            if end in ['end', 'both']: extra['end_point'] = [0., 0., 7.]
            add(end, **extra)
        for tolerance in [.001, .01, .03, .1, .3, .6, 1., 2.]:
            add('tolerance-' + str(tolerance), refit_tolerance=tolerance)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_candidates', request(), ['generate_loft_refit_candidates.py',
        'generate_loft_refit_coarse.py', 'generate_loft_refit_policy.py', 'generate_loft_refit.py',
        'generate_loft_rebuild.py', 'fixtures/loft.json', 'rhino_worker.py', 'client.py',
        'structural_sdk.py'], 'VibocerosOracleLoftRefitCandidates20261025')
