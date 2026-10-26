"""Document commands for newly admitted multispan Refit profiles."""
import copy
import json
from .capture_family import HERE, capture


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_multispan.json').read_text())
    operations = []
    for name in ['bend-1-0-plain', 'bend-2-1-domain', 'curvature-jump-frame']:
        original = next(o for o in query['operations'] if
                        o['id'] == 'loft-refit-multispan-' + name + '-0.001')
        for style in ['normal', 'straight']:
            op = copy.deepcopy(original)
            op['id'] = 'loft-refit-multispan-command-' + name + '-' + style
            op['style'] = style
            op['command'] = True
            operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_multispan_commands', request(), [
        'generate_loft_refit_multispan_commands.py', 'fixtures/loft_refit_multispan.json',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitMultispanCommands20261026')
