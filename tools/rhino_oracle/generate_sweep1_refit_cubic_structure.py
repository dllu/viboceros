"""Isolate public cubic fitting, derivative breaks and local interval re-entry."""
import copy
import json
import math

from .capture_family import capture
from .generate_sweep1_refit_stage_reentry import HERE

FAMILY = 'sweep1_refit_cubic_structure'
INPUTS = 'fixtures/sweep1_refit_representations.json'


def request(angle=0.):
    inputs = json.loads((HERE / INPUTS).read_text())
    operations = []
    for sweep in inputs['operations']:
        curve = sweep['sections'][0]
        op = dict(id=sweep['id'], op='curve_cubic_structure',
                  curve={key: copy.deepcopy(curve[key])
                         for key in ['degree', 'knots', 'control_points']},
                  distance=sweep['refit_tolerance'], angle=angle)
        operations.append(op)
        if sweep['id'].endswith('-original'):
            for a, b in [(0., .125), (0., .0625), (.125, .25), (.25, .5),
                         (.5, .5625), (.5625, .59375), (.75, .875), (.875, 1.)]:
                trimmed = copy.deepcopy(op)
                trimmed.update(id=op['id'] + '-{}-{}'.format(a, b), lookup_interval=[a, b])
                operations.append(trimmed)
    return dict(protocol_version=1, iterations=1, tolerance=inputs['tolerance'],
                operations=operations)


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--default-angle', action='store_true')
    args = parser.parse_args()
    family = FAMILY + '_default_angle' if args.default_angle else FAMILY
    capture(family, request(math.pi / 180. if args.default_angle else 0.), [
        'generate_sweep1_refit_cubic_structure.py', INPUTS,
        'curve_cubic_structure_probe.py', 'geometry_validity_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleSweepRefitCubicStructure20261029' + ('DefaultAngle' if args.default_angle else ''),
        timeout=300)
