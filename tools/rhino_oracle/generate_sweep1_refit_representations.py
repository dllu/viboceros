"""Separate input representation from geometric subdivision on C1 profiles.

Capture public degree elevation and knot insertion with --inputs first; their
returned definitions become frozen inputs to the subsequent Sweep1 capture.
"""
import argparse
import copy
import json

from .capture_family import capture
from .generate_sweep1_refit_stage_reentry import HERE, PROFILES
from .generate_sweep1_refit import request as base_request

INPUT_FAMILY = 'sweep1_refit_representation_inputs'
FAMILY = 'sweep1_refit_representations'


def input_request():
    operations = []
    for profile in json.loads((HERE / PROFILES).read_text())['profiles']:
        for variant in ['degree4', 'inserted-knot']:
            operation = dict(id=profile['id'] + '-' + variant,
                             curve=copy.deepcopy(profile['curve']))
            if variant == 'degree4':
                operation.update(op='curve_change_degree_geometry', degree=4,
                                 deformable=False)
            else:
                operation.update(op='curve_insert_knot_geometry',
                                 parameter=.3, multiplicity=1)
            operations.append(operation)
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-7, relative=1e-10, angular=1e-10),
                operations=operations)


def request():
    definitions = json.loads((HERE / ('observations/' + INPUT_FAMILY + '.json')).read_text())
    definitions = {row['id']: row['value'] for row in definitions['results']}
    root = base_request()
    template = root['operations'][0]
    operations = []
    for profile in json.loads((HERE / PROFILES).read_text())['profiles']:
        for variant in ['original', 'affine-domain', 'degree4', 'inserted-knot']:
            curve = copy.deepcopy(profile['curve'] if variant in ['original', 'affine-domain']
                                  else definitions[profile['id'] + '-' + variant])
            curve['type'] = 'nurbs'
            if variant == 'affine-domain':
                curve['knots'] = [-7. + 20. * knot for knot in curve['knots']]
            operation = copy.deepcopy(template)
            operation.update(id=profile['id'] + '-' + variant, sections=[curve],
                             parameters=[0.], refit_tolerance=profile['tolerance'])
            operations.append(operation)
    root['operations'] = operations
    return root


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inputs', action='store_true')
    args = parser.parse_args()
    sources = ['generate_sweep1_refit_representations.py', PROFILES,
               'rhino_worker.py', 'client.py', 'structural_sdk.py']
    if args.inputs:
        capture(INPUT_FAMILY, input_request(), sources,
                'VibocerosOracleSweepRefitRepresentationsInputs20261028', timeout=300)
    else:
        capture(FAMILY, request(), sources + [
            'observations/' + INPUT_FAMILY + '.json',
            'generate_sweep1_refit.py', 'generate_sweep1_rebuild.py',
            'sweep1_probe.py'],
            'VibocerosOracleSweepRefitRepresentations20261028', timeout=300)
