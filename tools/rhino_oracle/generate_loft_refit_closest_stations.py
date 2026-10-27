"""Public closest queries on saved endpoint-frame cubic fitting stations."""
import copy
import json
from .capture_family import HERE, capture


def request():
    inputs = json.loads((HERE / 'fixtures/loft_refit_closest_stations_inputs.json').read_text())
    operations = []
    for case in inputs['cases']:
        for index, target in enumerate(case['targets'], 1):
            operation = copy.deepcopy(case['candidate'])
            operation.update(op='nurbs_curve_closest_point',
                id=case['id'] + '-station-' + str(index), target=target)
            operations.append(operation)
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_refit_closest_stations', request(), [
        'generate_loft_refit_closest_stations.py',
        'fixtures/loft_refit_closest_stations_inputs.json',
        'fixtures/loft_refit_join_modes.json', 'observations/loft_refit_join_modes.json',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitClosestStations20261027')
