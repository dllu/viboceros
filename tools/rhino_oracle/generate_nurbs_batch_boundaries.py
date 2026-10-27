"""Measure numerical batch boundary choices separately from precise inverses."""
import copy
from .capture_family import capture
from .generate_loft_refit_midpoint_clocks import request as original


def request():
    query = original()
    template = query['operations'][0]
    operations = []

    def append(offset, reverse, accuracy, variant='ordinary'):
        op = copy.deepcopy(template)
        op.update(id='nurbs-batch-boundary-{}-{}-{}-{}'.format(
            offset, int(reverse), accuracy, variant), refit_tolerance=0.,
            refit_adjustment_fractional_tolerance=accuracy)
        ratio = 1. + offset
        for curve in op['curves']:
            z = curve['control_points'][0]['point'][2]
            points = [[0., 0., z], [1., 0., z], [1. + ratio, ratio * 1e-5, z]]
            if reverse:
                points.reverse()
            for control, point in zip(curve['control_points'], points):
                if variant in ['small', 'large']:
                    point = [c * (1. / 1024. if variant == 'small' else 1024.) for c in point]
                if variant == 'linear':
                    point[1] = 0.
                control['point'] = point
            if variant == 'domain':
                curve['knots'] = [-7. + 16. * t for t in curve['knots']]
        operations.append(op)

    for offset in [1e-7, 1e-6, 1e-5, 1e-4, 2e-4, 1e-3]:
        for reverse in [False, True]:
            for accuracy in [1e-5, 1e-3, 5e-3, 2e-2]:
                append(offset, reverse, accuracy)
    for offset in [1e-6, 1e-4]:
        for reverse in [False, True]:
            for variant in ['small', 'large', 'domain', 'linear']:
                append(offset, reverse, 1e-3, variant)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('nurbs_batch_boundaries', request(), [
        'generate_nurbs_batch_boundaries.py', 'generate_loft_refit_midpoint_clocks.py',
        'generate_loft_refit_midpoint_snapping.py', 'fixtures/loft_refit_unequal_sampling.json',
        'loft_refit_adjustment_probe.py', 'loft_scalar_clock_probe.py', 'loft_refit_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleNurbsBatchBoundaries20261027')
