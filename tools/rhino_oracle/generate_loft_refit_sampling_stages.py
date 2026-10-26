"""Inspect public cubic pieces and simplification before joined fitting."""
import copy
import json
from .capture_family import HERE, capture


def request():
    base = json.loads((HERE / 'fixtures/loft_refit_adaptive_lines.json').read_text())
    intervals = json.loads((HERE / 'fixtures/loft_refit_interval_sampling.json').read_text())
    names = ['loft-refit-adaptive-unequal-5e-07', 'loft-refit-adaptive-unequal-1e-06',
             'loft-refit-adaptive-two-bends-5e-07', 'loft-refit-adaptive-two-bends-1e-06',
             'loft-refit-interval-sampling-0.75-0-0.0', 'loft-refit-interval-sampling-0.375-0-0.0']
    rows = {op['id']: op for op in base['operations'] + intervals['operations']}
    base['operations'] = []
    for name in names:
        op = copy.deepcopy(rows[name])
        op['id'] = name.replace('loft-refit-', 'loft-refit-sampling-stage-', 1)
        op['inspect_refit_beziers'] = True
        op['inspect_refit_sources'] = True
        op['inspect_refit_adjustment'] = True
        base['operations'].append(op)
    return base


if __name__ == '__main__':
    capture('loft_refit_sampling_stages', request(),
        ['generate_loft_refit_sampling_stages.py', 'fixtures/loft_refit_adaptive_lines.json',
         'fixtures/loft_refit_interval_sampling.json', 'loft_refit_probe.py',
         'loft_refit_source_probe.py', 'loft_refit_adjustment_probe.py',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitSamplingStages20261026')
