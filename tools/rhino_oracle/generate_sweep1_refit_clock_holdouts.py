"""Independent spatial controls around the batch policy transition."""
import copy
import math

from .capture_family import capture
from .generate_sweep1_refit_clock_resolutions import request as resolutions

FAMILY = 'sweep1_refit_clock_holdouts'


def request():
    root = resolutions()
    operations = []
    for operation in root['operations']:
        operation = copy.deepcopy(operation)
        operation['id'] += '-spatial'
        for control in operation['curve']['control_points']:
            x, y, z = control['point']
            control['point'] = [z, x, y]
        operation['inspect_numerical_batch'] = [
            dict(id='holdout-' + str(index), fractional_tolerance=accuracy, history=[])
            for index, accuracy in enumerate([
                math.nextafter(1e-5, 0.), math.nextafter(1e-5, 1.), .02, .1])]
        operations.append(operation)
    root['operations'] = operations
    return root


if __name__ == '__main__':
    capture(FAMILY, request(), [
        'generate_sweep1_refit_clock_holdouts.py',
        'generate_sweep1_refit_clock_resolutions.py',
        'generate_sweep1_refit.py', 'generate_sweep1_rebuild.py',
        'capture_sources/sweep1_refit_c1_profiles.json',
        'arc_length_batch_probe.py', 'geometry_validity_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleSweepRefitClockHoldouts20261028', timeout=300)
