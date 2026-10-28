"""Re-enter constructed C1 profiles through the public Sweep1 SDK."""
import copy
import json
from pathlib import Path

from .capture_family import capture
from .generate_sweep1_refit import request as base_request

HERE = Path(__file__).resolve().parent
PROFILES = 'capture_sources/sweep1_refit_c1_profiles.json'


def request():
    root = base_request()
    template = copy.deepcopy(root['operations'][0])
    root['operations'] = []
    for profile in json.loads((HERE / PROFILES).read_text())['profiles']:
        for mode in ['single', 'pair', 'disabled']:
            first = copy.deepcopy(profile['curve'])
            sections = [first]
            if mode == 'pair':
                second = copy.deepcopy(first)
                for control in second['control_points']:
                    control['point'][2] += 5.
                sections.append(second)
            op = copy.deepcopy(template)
            op.update(id=profile['id'] + '-c1-' + mode, sections=sections,
                      parameters=[0., 5.] if mode == 'pair' else [0.],
                      refit_tolerance=0. if mode == 'disabled' else profile['tolerance'])
            root['operations'].append(op)
    return root


if __name__ == '__main__':
    capture('sweep1_refit_stage_reentry', request(), [
        'generate_sweep1_refit_stage_reentry.py', PROFILES,
        'generate_sweep1_refit.py', 'generate_sweep1_rebuild.py',
        'sweep1_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleSweepRefitStageReentry20261028', timeout=300)
