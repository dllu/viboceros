"""Use the Loft default angle for independent public preparation probes."""
import math
from .capture_family import capture
from .generate_loft_refit_adaptive_preparation import request as preparation


def request():
    query = preparation()
    query['tolerance']['angular'] = math.radians(1.)
    for op in query['operations']:
        op['id'] = op['id'].replace('adaptive-preparation-', 'adaptive-default-angle-')
        op['refit_angle_tolerance'] = math.radians(1.)
    return query


if __name__ == '__main__':
    capture('loft_refit_adaptive_default_angle', request(),
        ['generate_loft_refit_adaptive_default_angle.py',
         'generate_loft_refit_adaptive_preparation.py',
         'fixtures/loft_refit_adaptive_lines.json', 'loft_refit_frame_probe.py',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitAdaptiveDefaultAngle20261026')
