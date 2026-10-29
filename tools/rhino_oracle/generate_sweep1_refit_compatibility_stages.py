"""Compare public MakeCompatible stages on original and reconstructed profiles."""
import copy
import json
import math

from .capture_family import capture
from .generate_sweep1_refit_stage_reentry import HERE, PROFILES

FAMILY = 'sweep1_refit_compatibility_stages'


def request():
    profiles = json.loads((HERE / PROFILES).read_text())['profiles']
    originals = {}
    for family in ['sweep1_refit_sdk', 'sweep1_refit_sdk_holdouts']:
        for op in json.loads((HERE / ('fixtures/' + family + '.json')).read_text())['operations']:
            originals[op['id']] = op
    operations = []
    for profile in profiles:
        for stage in ['original', 'c1']:
            curve = profile['curve'] if stage == 'c1' else originals[profile['id']]['sections'][0]
            curve = {key: copy.deepcopy(curve[key]) for key in ['degree', 'knots', 'control_points']}
            other = copy.deepcopy(curve)
            for control in other['control_points']:
                control['point'][2] += 5.
            for angle in [0., math.pi / 180.]:
                operations.append(dict(op='loft', id=profile['id'] + '-' + stage + '-angle-' + str(angle),
                    curves=[curve, other], style='straight', closed=False,
                    refit_tolerance=profile['tolerance'], refit_angle_tolerance=angle,
                    inspect_inputs=True, sample_geometry=True, allow_empty=True,
                    inspect_refit_sources=True))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-7, relative=1e-10, angular=1e-10), operations=operations)


if __name__ == '__main__':
    capture(FAMILY, request(), [
        'generate_sweep1_refit_compatibility_stages.py', PROFILES,
        'fixtures/sweep1_refit_sdk.json', 'fixtures/sweep1_refit_sdk_holdouts.json',
        'loft_refit_source_probe.py', 'loft_refit_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleSweepRefitCompatibilityStages20261029', timeout=300)
