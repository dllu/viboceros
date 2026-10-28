"""Public closest-point calibration on independent fitting proposals."""
import copy
import json
from pathlib import Path

from .capture_family import capture

HERE = Path(__file__).resolve().parent
INPUTS = 'capture_sources/sweep1_refit_closest_inputs.json'


def request():
    operations = []
    for candidate in json.loads((HERE / INPUTS).read_text())['inputs']:
        for station, target in enumerate(candidate['targets'], 1):
            operation = copy.deepcopy(candidate['curve'])
            operation.update(id=candidate['id'] + '-' + str(station),
                             op='nurbs_curve_closest_point', target=target)
            operations.append(operation)
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-7, relative=1e-10, angular=1e-10),
                operations=operations)


if __name__ == '__main__':
    capture('sweep1_refit_closest', request(), [
        'generate_sweep1_refit_closest.py', INPUTS,
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleSweepRefitClosest20261028', timeout=300)
