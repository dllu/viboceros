"""Independent scalar length inverses across natural knots and plateaus."""
import copy
from .capture_family import capture


def request():
    bend = [[0., 0., 0.], [1., 0., 0.], [2.00001, 1.00001e-5, 0.]]
    cases = [
        ('plain', bend, [0., 0., 1., 2., 2.], 1.),
        ('reverse', list(reversed(bend)), [-7., -7., 3., 13., 13.], 1.),
        ('frame', [[7. + y, -3., 2. + x] for x, y, z in bend], [100., 100., 107., 114., 114.], 1.),
        ('collinear', [[0., 0., 0.], [1., 0., 0.], [2.00001, 0., 0.]], [0., 0., 1., 2., 2.], 1.),
        ('orthogonal', [[0., 0., 0.], [1., 0., 0.], [1., 1.00001, 0.]], [0., 0., .2, 1., 1.], 1.),
        ('plateau', [[0., 0., 0.], [1., 0., 0.], [1., 0., 0.], [2.00001, 0., 0.]], [-3., -3., -1., 2., 7., 7.], 1.),
        ('negative', bend, [0., 0., 1., 2., 2.], -2.),
        ('large', [[1000. * x for x in p] for p in bend], [0., 0., 1., 2., 2.], 1.),
        ('small', [[.001 * x for x in p] for p in bend], [0., 0., 1., 2., 2.], 1.),
    ]
    operations = []
    fractions = [0., 1e-7, .125, .25, .49999, .5, .50001, .75, .875, 1. - 1e-7, 1.]
    for name, points, knots, weight in cases:
        curve = dict(degree=1, knots=knots, control_points=[dict(point=p, weight=weight) for p in points])
        for accuracy in [1e-8, 1e-6, 1e-4, 1e-2]:
            operations.append(dict(op='curve_length_parameters', id='scalar-knot-holdout-{}-{}'.format(name, accuracy),
                curve=copy.deepcopy(curve), fractions=fractions[:], fractional_tolerance=accuracy))
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('scalar_knot_holdouts', request(),
        ['generate_scalar_knot_holdouts.py', 'loft_scalar_clock_probe.py',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleScalarKnotHoldouts20261026')
