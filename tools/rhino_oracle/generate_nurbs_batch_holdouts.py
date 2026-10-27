"""Boundary and interior holdouts for constant-speed numerical NURBS batches."""
import math
from .capture_family import capture


def request():
    operations = []
    accuracies = [1e-6, 9.999e-6, 1e-5, 1.0001e-5, 1e-4, .009999, .01, .010001, .02]
    for variant in range(8):
        scale = [1., 1., 1. / 1024., 1024., 1., 1., 1., 1.][variant]
        points = [[0., 0., 0.], [.37, .05, .1], [1.5, -.1, .3], [2., 1., -.2]]
        knots = [0., 0., .3, .7, 1., 1.]
        if variant == 1:
            points.reverse()
        if variant == 4:
            knots = [-7. + 16. * t for t in knots]
        if variant == 5:
            knots = [10., 10., 11., 20., 90., 90.]
        if variant == 7:
            points = [[0., 0., 0.], [4., .2, .1], [5., 1., -.3], [6., .4, 1.]]
        points = [[coordinate * scale for coordinate in point] for point in points]
        lengths = [math.dist(a, b) for a, b in zip(points, points[1:])]
        total = math.fsum(lengths)
        boundaries = [lengths[0] / total, math.fsum(lengths[:2]) / total]
        # Interior checkpoints distinguish natural-boundary retention from the
        # independent scalar inverse's denser quadrature trace.
        interior = [.06 * lengths[0] / total, .31 * lengths[0] / total]
        for index, accuracy in enumerate(accuracies):
            band = accuracy if accuracy < 1e-5 else min(accuracy, .01) * .01
            fractions = sorted([0., .125, .5, .875, 1.] + [
                fraction + factor * band for fraction in boundaries + interior
                for factor in [-1.1, -.9, 0., .9, 1.1]])
            weight = 1024. if variant == 6 else 1.
            operations.append(dict(op='arc_length_batch',
                id='nurbs-batch-holdout-{}-{}'.format(variant, index),
                curve=dict(degree=1, knots=knots,
                    control_points=[dict(point=p, weight=weight) for p in points]),
                fractions=fractions, inspect_numerical_batch=[
                    dict(id='cold', fractional_tolerance=accuracy, history=[])]))
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('nurbs_batch_holdouts', request(), [
        'generate_nurbs_batch_holdouts.py', 'arc_length_batch_probe.py',
        'geometry_validity_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleNurbsBatchHoldouts20261027')
