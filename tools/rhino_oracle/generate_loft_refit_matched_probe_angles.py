"""Native inspections inherit implicit and explicit Loft fitting angles."""
import copy
import math
from .capture_family import capture
from .generate_loft_refit_adaptive_preparation import request as preparation


def request():
    original = preparation()
    operations = []
    for explicit in [None, math.pi / 180.]:
        for source in original['operations']:
            op = copy.deepcopy(source)
            op['id'] = op['id'].replace('adaptive-preparation-',
                'matched-probe-angles-' + ('default-' if explicit is None else 'explicit-'))
            if explicit is not None:
                op['refit_angle_tolerance'] = explicit
            operations.append(op)
    for source in original['operations'][:2]:
        op = copy.deepcopy(source)
        op['id'] = op['id'].replace('adaptive-preparation-', 'matched-probe-angles-small-')
        op['refit_angle_tolerance'] = 1e-8
        operations.append(op)
    original['operations'] = operations
    return original


if __name__ == '__main__':
    capture('loft_refit_matched_probe_angles', request(),
        ['generate_loft_refit_matched_probe_angles.py',
         'generate_loft_refit_adaptive_preparation.py',
         'fixtures/loft_refit_adaptive_lines.json', 'loft_refit_frame_probe.py',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitMatchedProbeAngles20261026')
