"""Coarse endpoint-tangent fits may smooth explicitly suppressed larger corners."""
import math
from .capture_family import capture


def request():
    operations = []
    for degrees in [2., 10., 30.]:
        bend = math.radians(degrees)
        points = [[0., 0.], [1., 0.], [1. + math.cos(bend), math.sin(bend)]]
        for factor in [.5, 1.5]:
            for limit in [.01, .1]:
                curves = [dict(degree=1, knots=[0., 0., 1., 2., 2.],
                    control_points=[dict(point=[p[0], p[1], z], weight=1.) for p in points])
                    for z in [0., 2., 4.]]
                operations.append(dict(op='loft', id='loft-refit-suppressed-initial-{}-{}-{}'.format(degrees, factor, limit),
                    curves=curves, style='straight', closed=False, refit_tolerance=limit,
                    refit_angle_tolerance=bend * factor, inspect_inputs=True, sample_geometry=True, allow_empty=True))
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_refit_suppressed_initial', request(),
        ['generate_loft_refit_suppressed_initial.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitSuppressedInitial20261026')
