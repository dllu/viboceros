"""Compare standalone fitting with public multi-profile preparation."""
import copy
import json
from .capture_family import HERE, capture


def request():
    original = json.loads((HERE / 'fixtures/loft_refit_adaptive_lines.json').read_text())
    names = ['angle-0.25-1.0-1.0', 'angle-0.25-1000.0-1.0',
             'angle-0.05-1000.0-1.0', 'threshold-0.003-1e-06']
    operations = []
    for name in names:
        op = copy.deepcopy(next(o for o in original['operations'] if
            o['id'] == 'loft-refit-adaptive-' + name))
        op['id'] = 'loft-refit-adaptive-preparation-' + name
        op['inspect_refitted'] = True
        op['inspect_refit_frames'] = True
        operations.append(op)
    original['operations'] = operations
    return original


if __name__ == '__main__':
    capture('loft_refit_adaptive_preparation', request(),
        ['generate_loft_refit_adaptive_preparation.py',
         'fixtures/loft_refit_adaptive_lines.json', 'loft_refit_frame_probe.py',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitAdaptivePreparation20261026')
