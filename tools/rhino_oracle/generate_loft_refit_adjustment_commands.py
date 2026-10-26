"""Command records for corrected rational and polynomial Refit profiles."""
import copy
import json
from .capture_family import HERE, capture


def request():
    query = json.loads((HERE / 'fixtures/loft_refit.json').read_text())
    quarter = next(o for o in query['operations'] if o['id'] == 'loft-refit-quarter-0.1-normal')
    fitting = json.loads((HERE / 'fixtures/loft_refit_adjustment_fit_clock.json').read_text())
    quadratic = next(o for o in fitting['operations'] if o['id'] == 'loft-refit-fit-clock-quadratic-0.03-rational')
    operations = []
    for shape, original in [('quarter', quarter), ('quadratic', quadratic)]:
        for style in ['normal', 'loose', 'tight', 'straight', 'uniform']:
            op = copy.deepcopy(original)
            for key in list(op):
                if key.startswith('inspect_') or key.startswith('refit_adjustment_'):
                    del op[key]
            op.update(id='loft-refit-adjustment-command-' + shape + '-' + style,
                      style=style, command=True)
            operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_adjustment_commands', request(), [
        'generate_loft_refit_adjustment_commands.py', 'fixtures/loft_refit.json',
        'fixtures/loft_refit_adjustment_fit_clock.json', 'rhino_worker.py',
        'client.py', 'structural_sdk.py'], 'VibocerosOracleLoftRefitAdjustmentCommands20261025')
