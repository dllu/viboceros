"""Initial multispan fits across representations, styles and document commands."""
import copy
import json
from .capture_family import HERE, capture
from .generate_loft_refit_source_continuity import elevated


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_interval_clocks.json').read_text())
    original = next(op for op in query['operations']
                    if op['id'] == 'loft-refit-interval-clock-0.75-0.2-0')
    operations = []
    for variant in ['plain', 'cubic', 'negative', 'frame', 'reversed']:
        transformed = copy.deepcopy(original)
        if variant == 'cubic':
            transformed['curves'] = [elevated(c, 3) for c in transformed['curves']]
        for curve in transformed['curves']:
            if variant == 'negative':
                for control in curve['control_points']:
                    control['weight'] = -2.
            elif variant == 'frame':
                for control in curve['control_points']:
                    x, y, z = control['point']
                    control['point'] = [7. + y, -3. + z, 2. + x]
            elif variant == 'reversed':
                curve['control_points'].reverse()
                curve['knots'] = [1. - k for k in reversed(curve['knots'])]
        for style in ['normal', 'loose', 'tight', 'straight', 'uniform']:
            for command in [False, True]:
                op = copy.deepcopy(transformed)
                op['id'] = 'loft-refit-initial-stations-{}-{}-{}'.format(
                    variant, style, 'command' if command else 'sdk')
                op['style'] = style
                op['command'] = command
                operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_initial_stations', request(), [
        'generate_loft_refit_initial_stations.py',
        'generate_loft_refit_source_continuity.py', 'fixtures/loft_refit_interval_clocks.json',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitInitialStations20261027')
