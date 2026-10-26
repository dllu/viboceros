"""Resolve the observed shallow-corner boundary without guessing an angle floor."""
import math
from .capture_family import capture


def request():
    operations = []
    for radians in [.0018, .0019, .00199, .002, .00200001, .00201, .0021, .0022, .0023, .0024, .0025]:
        for scale in [1., 1000.]:
            points = [[0., 0.], [1., 0.], [1. + math.cos(radians), math.sin(radians)]]
            curves = [dict(degree=1, knots=[0., 0., 1., 2., 2.],
                control_points=[dict(point=[scale * p[0], scale * p[1], scale * z], weight=1.)
                    for p in points]) for z in [0., 2., 4.]]
            operations.append(dict(op='loft', id='loft-refit-corner-boundary-{}-{}'.format(radians, scale),
                curves=curves, style='straight', closed=False, refit_tolerance=1e-6 * scale,
                refit_angle_tolerance=1e-8, inspect_inputs=True, sample_geometry=True, allow_empty=True))
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_refit_corner_boundary', request(),
        ['generate_loft_refit_corner_boundary.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitCornerBoundary20261026')
