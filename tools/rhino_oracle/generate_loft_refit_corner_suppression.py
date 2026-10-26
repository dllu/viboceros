"""Requested angles above the intrinsic corner floor retain their own boundary."""
import math
from .capture_family import capture


def request():
    operations = []
    for bend in [.00201, .0021, .0022, .003, .004]:
        points = [[0., 0.], [1., 0.], [1. + math.cos(bend), math.sin(bend)]]
        for angle in [.001, .00205, .00215, .0025, .0035]:
            curves = [dict(degree=1, knots=[0., 0., 1., 2., 2.],
                control_points=[dict(point=[p[0], p[1], z], weight=1.) for p in points])
                for z in [0., 2., 4.]]
            operations.append(dict(op='loft', id='loft-refit-corner-suppression-{}-{}'.format(bend, angle),
                curves=curves, style='straight', closed=False, refit_tolerance=1e-6,
                refit_angle_tolerance=angle, inspect_inputs=True, sample_geometry=True, allow_empty=True))
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_refit_corner_suppression', request(),
        ['generate_loft_refit_corner_suppression.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitCornerSuppression20261026')
