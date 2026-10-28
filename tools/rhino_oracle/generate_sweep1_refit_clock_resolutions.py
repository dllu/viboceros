"""Calibrate fresh single- and multi-span public length batches by accuracy."""
import copy
import json

from .capture_family import capture
from .generate_sweep1_refit import request as base_request
from .generate_sweep1_refit_stage_reentry import HERE, PROFILES

FAMILY = 'sweep1_refit_clock_resolutions'


def request():
    profiles = json.loads((HERE / PROFILES).read_text())['profiles']
    source = base_request()['operations'][0]['sections'][0]
    inputs = [(p['id'], p['curve']) for p in profiles] + [('single-quadratic', source)]
    scenarios = [dict(id='accuracy-' + str(i), fractional_tolerance=accuracy, history=[])
                 for i, accuracy in enumerate([1e-8, 1e-6, 1e-5, 1e-4, 1e-3, 1e-2])]
    operations = []
    for name, curve in inputs:
        operations.append(dict(op='arc_length_batch', id=name.replace('.', 'p'),
                               curve={key: copy.deepcopy(curve[key])
                                      for key in ['degree', 'knots', 'control_points']},
                               fractions=[i / 8. for i in range(9)],
                               inspect_numerical_batch=copy.deepcopy(scenarios)))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-7, relative=1e-10, angular=1e-10),
                operations=operations)


if __name__ == '__main__':
    capture(FAMILY, request(), [
        'generate_sweep1_refit_clock_resolutions.py',
        'generate_sweep1_refit.py', 'generate_sweep1_rebuild.py',
        PROFILES, 'arc_length_batch_probe.py', 'geometry_validity_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleSweepRefitClockResolutions20261028', timeout=300)
