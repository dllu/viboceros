"""Replay section fitting directly, including disabled-fitting controls."""
import copy
import json
import math
import sys

from .capture_family import capture
from .generate_sweep1_refit_stage_reentry import HERE

FAMILY = 'sweep1_refit_sections'
INPUTS = 'fixtures/sweep1_refit_compatibility_stages.json'


def request():
    inputs = json.loads((HERE / INPUTS).read_text())
    operations = []
    for old in inputs['operations'][1::2]:
        name = old['id'].split('-angle-')[0]
        for count in [1, 2]:
            operations.append(dict(op='sweep_refit_sections', id=name + '-count-' + str(count),
                sections=copy.deepcopy(old['curves'][:count]), parameters=[5. * i for i in range(count)],
                refit_tolerance=old['refit_tolerance']))
    line = dict(degree=1, knots=[0., 0., 1., 1.],
                control_points=[dict(point=p, weight=1.) for p in [[0., 0., 0.], [1., 0., 0.]]])
    second = copy.deepcopy(line)
    for p in second['control_points']:
        p['point'][2] += 5.
    for value in [0., 2. * math.sqrt(sys.float_info.epsilon)]:
        operations.append(dict(op='sweep_refit_sections', id='disabled-lines-' + str(value),
            sections=[line, second], parameters=[-7., 13.], refit_tolerance=value))
    circle = next(op['sections'][0] for op in operations if op['id'] == 'nurbs-circle-1e-06-original-count-1')
    operations.append(dict(op='sweep_refit_sections', id='disabled-circle', sections=[copy.deepcopy(circle)],
                           parameters=[17.], refit_tolerance=0.))
    return dict(protocol_version=1, iterations=1, tolerance=inputs['tolerance'], operations=operations)


if __name__ == '__main__':
    capture(FAMILY, request(), [
        'generate_sweep1_refit_sections.py', INPUTS, 'sweep_refit_sections_probe.py',
        'geometry_validity_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleSweepRefitSections20261029', timeout=300)
