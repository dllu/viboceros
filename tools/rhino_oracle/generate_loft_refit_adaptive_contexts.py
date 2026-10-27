"""Adaptive regions through document commands, styles and source frames."""
import copy
import json
from .capture_family import HERE, capture


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_join_modes.json').read_text())
    cases = [('smooth', 'smooth-1e-05-0.0625'),
             ('rational', 'rational-1e-05-1.0'),
             ('spatial', 'spatial-1e-05-1.0'),
             ('quintic', 'quintic-1e-05-0.25')]
    operations = []
    for name, suffix in cases:
        original = copy.deepcopy(next(op for op in query['operations']
            if op['id'] == 'loft-refit-join-modes-' + suffix))
        original.pop('inspect_refit_sources', None)
        for style in ['normal', 'loose', 'tight', 'straight', 'uniform']:
            for command in [False, True]:
                op = copy.deepcopy(original)
                op.update(id='loft-refit-adaptive-contexts-{}-{}-{}'.format(name, style, int(command)),
                          style=style, command=command)
                operations.append(op)
        for variant in ['frame', 'reversed', 'gauge', 'domain', 'small', 'large']:
            op = copy.deepcopy(original)
            op['id'] = 'loft-refit-adaptive-contexts-{}-{}'.format(name, variant)
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
    capture('loft_refit_adaptive_contexts', request(), [
        'generate_loft_refit_adaptive_contexts.py', 'fixtures/loft_refit_join_modes.json',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitAdaptiveContexts20261027')
