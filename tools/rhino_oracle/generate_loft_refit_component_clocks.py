"""Compare original and simplified source clocks around paired subdivision."""
import copy
import json
from .capture_family import HERE, capture


def request():
    base = json.loads((HERE / 'fixtures/loft_refit_midpoint_boundary.json').read_text())
    selected = [o for o in base['operations'] if o['id'] in [
        'loft-refit-midpoint-boundary--0.02-0', 'loft-refit-midpoint-boundary-0.02-1']]
    unequal = json.loads((HERE / 'fixtures/loft_refit_unequal_sampling.json').read_text())
    selected += [o for o in unequal['operations'] if o['id'] in [
        'loft-refit-unequal-sampling-1.0-1-1e-06',
        'loft-refit-unequal-sampling-2.0-0-2.5e-07',
        'loft-refit-unequal-sampling-2.0-0-5e-07',
        'loft-refit-unequal-sampling-2.0-0-1e-06']]
    operations = []
    for original in selected:
        op = copy.deepcopy(original)
        op['id'] = original['id'].replace('loft-refit-', 'loft-refit-component-clock-', 1)
        op.pop('inspect_scalar_clock', None)
        op['inspect_refit_sources'] = True
        operations.append(op)
    base['operations'] = operations
    return base


if __name__ == '__main__':
    capture('loft_refit_component_clocks', request(),
        ['generate_loft_refit_component_clocks.py', 'fixtures/loft_refit_midpoint_boundary.json',
         'fixtures/loft_refit_unequal_sampling.json', 'loft_refit_source_probe.py',
         'loft_refit_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitComponentClocks20261026')
