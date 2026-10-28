"""Separate SDK sweep tolerance from the section Refit tolerance."""
import copy
import json

from .capture_family import capture
from .generate_sweep1_refit import request as base_request
from .generate_sweep1_refit_stage_reentry import HERE, PROFILES


def request():
    root = base_request()
    original = {op['id']: op for op in root['operations']}
    template = copy.deepcopy(root['operations'][0])
    profiles = json.loads((HERE / PROFILES).read_text())['profiles']
    inputs = [copy.deepcopy(original[name]) for name in
              ['open-refit-1e-06', 'closed-refit-1e-06']]
    for profile in profiles:
        if profile['id'] not in ['open-refit-1e-06', 'nurbs-circle-1e-06', 'closed-cubic-0.001']:
            continue
        op = copy.deepcopy(template)
        op.update(id=profile['id'] + '-c1', sections=[profile['curve']],
                  refit_tolerance=profile['tolerance'])
        inputs.append(op)
    root['operations'] = []
    for source in inputs:
        for tolerance in [1e-9, 1e-7, 1e-5]:
            op = copy.deepcopy(source)
            op.update(id=source['id'] + '-sweep-' + str(tolerance),
                      sdk_sweep_tolerance=tolerance)
            root['operations'].append(op)
    return root


if __name__ == '__main__':
    capture('sweep1_refit_sweep_tolerance', request(), [
        'generate_sweep1_refit_sweep_tolerance.py',
        'generate_sweep1_refit_stage_reentry.py', PROFILES,
        'generate_sweep1_refit.py', 'generate_sweep1_rebuild.py',
        'sweep1_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleSweepRefitSweepTolerance20261028', timeout=300)
