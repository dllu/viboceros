"""Locate representation sensitivity in public fitting and length queries."""
import copy
import json

from .capture_family import capture
from .generate_sweep1_refit_representations import HERE

FAMILY = 'sweep1_refit_representation_diagnostics'
INPUTS = 'fixtures/sweep1_refit_representations.json'


def request():
    source = json.loads((HERE / INPUTS).read_text())
    operations = []
    for operation in source['operations']:
        curve = copy.deepcopy(operation['sections'][0])
        operations.append(dict(id=operation['id'] + '-fit', op='curve_fit_geometry',
                               curve=curve, degree=3,
                               fit_tolerance=operation['refit_tolerance'],
                               angle_tolerance_radians=0.))
        curve = {key: curve[key] for key in ['degree', 'knots', 'control_points']}
        operations.append(dict(id=operation['id'].replace('.', 'p') + '-length',
                               op='arc_length_batch', curve=curve,
                               fractions=[i / 8. for i in range(9)],
                               inspect_numerical_batch=[dict(
                                   id='fresh-1e-4', fractional_tolerance=1e-4,
                                   history=[])]))
    return dict(protocol_version=1, iterations=1, tolerance=source['tolerance'],
                operations=operations)


if __name__ == '__main__':
    capture(FAMILY, request(), [
        'generate_sweep1_refit_representation_diagnostics.py',
        'generate_sweep1_refit_representations.py', INPUTS,
        'arc_length_batch_probe.py', 'geometry_validity_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleSweepRefitRepresentationDiagnostics20261028', timeout=300)
