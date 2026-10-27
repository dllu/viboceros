"""Inspect fitting clocks at curved multispan initial-fit transitions."""
import copy
import json
from .capture_family import HERE, capture


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_initial_holdouts.json').read_text())
    operations = []
    for original in query['operations']:
        if '-curvature-jump-' not in original['id']:
            continue
        op = copy.deepcopy(original)
        op['id'] = op['id'].replace('initial-holdout-', 'curved-clock-')
        op['inspect_refit_sources'] = True
        operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_curved_clocks', request(), [
        'generate_loft_refit_curved_clocks.py', 'fixtures/loft_refit_initial_holdouts.json',
        'loft_refit_source_probe.py', 'loft_refit_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitCurvedClocks20261027')
