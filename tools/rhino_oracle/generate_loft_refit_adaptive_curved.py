"""Multi-piece curved Loft refits through styles, commands and chart changes."""
import copy
import json
from .capture_family import HERE, capture


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_initial_holdouts.json').read_text())
    sources = []
    for name, original_id, tolerance in [
            ('equal', 'curvature-jump-0-2e-06', 2e-6),
            ('unequal', 'curvature-jump-1-2e-06', 2e-6),
            ('tight', 'curvature-jump-0-2e-06', 1e-6)]:
        op = copy.deepcopy(next(op for op in query['operations']
            if op['id'] == 'loft-refit-initial-holdout-' + original_id))
        op['refit_tolerance'] = tolerance
        sources.append((name, op))
    operations = []
    for name, original in sources:
        for style in ['normal', 'loose', 'tight', 'straight', 'uniform']:
            for command in [False, True]:
                op = copy.deepcopy(original)
                op['id'] = 'loft-refit-adaptive-curved-{}-{}-{}'.format(name, style, int(command))
                op['style'], op['command'] = style, command
                operations.append(op)
        for variant in ['frame', 'reversed', 'gauge', 'domain', 'small', 'large']:
            op = copy.deepcopy(original)
            op['id'] = 'loft-refit-adaptive-curved-{}-{}'.format(name, variant)
            for i, curve in enumerate(op['curves']):
                if variant == 'reversed':
                    curve['control_points'].reverse()
                    curve['knots'] = [1. - k for k in reversed(curve['knots'])]
                elif variant == 'domain':
                    curve['knots'] = [-3. * i + 7. * (i + 1.) * k for k in curve['knots']]
                for control in curve['control_points']:
                    x, y, z = control['point']
                    if variant == 'frame':
                        control['point'] = [7. + y, -3. + z, 2. + x]
                    elif variant == 'gauge':
                        control['weight'] *= -2.
                    elif variant in ['small', 'large']:
                        scale = 1. / 16. if variant == 'small' else 16.
                        control['point'] = [c * scale for c in [x, y, z]]
            if variant in ['small', 'large']:
                op['refit_tolerance'] *= 1. / 16. if variant == 'small' else 16.
            operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_adaptive_curved', request(), [
        'generate_loft_refit_adaptive_curved.py', 'fixtures/loft_refit_initial_holdouts.json',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitAdaptiveCurved20261027')
