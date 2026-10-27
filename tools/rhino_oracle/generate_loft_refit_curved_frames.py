"""Exercise recovered curved initial fits through styles, commands and frames."""
import copy
import json
from .capture_family import HERE, capture


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_initial_holdouts.json').read_text())
    selected = [op for op in query['operations'] if
                op['id'] == 'loft-refit-initial-holdout-curvature-jump-0-1e-05'
                or ('-curvature-jump-1-' in op['id'] and op['refit_tolerance'] in [3e-6, 4e-6, 6e-6])]
    operations = []
    for original in selected:
        name = original['id'].removeprefix('loft-refit-initial-holdout-')
        for style in ['normal', 'loose', 'tight', 'straight', 'uniform']:
            for command in [False, True]:
                op = copy.deepcopy(original)
                op['id'] = 'loft-refit-curved-frame-{}-{}-{}'.format(name, style, int(command))
                op['style'], op['command'] = style, command
                operations.append(op)
        for variant in ['frame', 'reversed', 'gauge', 'domain', 'small', 'large']:
            op = copy.deepcopy(original)
            op['id'] = 'loft-refit-curved-frame-{}-{}'.format(name, variant)
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
    capture('loft_refit_curved_frames', request(), [
        'generate_loft_refit_curved_frames.py', 'fixtures/loft_refit_initial_holdouts.json',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitCurvedFrames20261027')
