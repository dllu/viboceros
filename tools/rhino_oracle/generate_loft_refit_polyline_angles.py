"""Locate the public Loft Refit corner threshold and retain mixed open profiles."""
import copy
import math
from .capture_family import capture
from .generate_loft_refit_polylines import polyline


def request():
    operations = []
    for angle in [.1, .5, .999, 1., 1.001, 1.5, 2., 5.]:
        points = [[0., 0., 0.], [1., 0., 0.], [1. + math.cos(math.radians(angle)), math.sin(math.radians(angle)), 0.]]
        for tolerance in [.001, 1e-6]:
            curves = [polyline(copy.deepcopy(points)) for _ in range(2)]
            for cp in curves[1]['control_points']: cp['point'][2] = 2.
            operations.append(dict(op='loft', id='loft-refit-corner-angle-{}-{}'.format(angle, tolerance),
                curves=curves, style='straight', closed=False, refit_tolerance=tolerance,
                inspect_inputs=True, sample_geometry=True, allow_empty=True))
    for counts in [(2, 3, 2), (3, 2, 4)]:
        curves = []
        for i, count in enumerate(counts):
            points = [[0., 0., 2. * i]]
            for j in range(count):
                p = list(points[-1]); p[j % 2] += 1. + .25 * i; points.append(p)
            curves.append(polyline(points))
        for style in ['normal', 'loose', 'tight', 'straight', 'uniform']:
            operations.append(dict(op='loft', id='loft-refit-open-counts-{}-{}'.format(''.join(map(str, counts)), style),
                curves=copy.deepcopy(curves), style=style, closed=False, refit_tolerance=.001,
                inspect_inputs=True, sample_geometry=True, allow_empty=True))
    return dict(protocol_version=1, iterations=1, tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_refit_polyline_angles', request(), ['generate_loft_refit_polyline_angles.py',
        'generate_loft_refit_polylines.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitPolylineAngles20261025')
