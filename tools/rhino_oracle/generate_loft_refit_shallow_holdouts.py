"""Minimum-angle boundary, rational clocks, spatial frames and region matching."""
import copy
import math
from .capture_family import capture


def curve(points, z, degree=1, weights=None):
    points = [[p[0], p[1], z] for p in points]
    if degree == 2:
        a, b, c = points
        mid = lambda p, q: [(x + y) * .5 for x, y in zip(p, q)]
        points = [a, mid(a, b), b, mid(b, c), c]
    knots = [0.] * (degree + 1) + [1.] * degree + [2.] * (degree + 1)
    return dict(degree=degree, knots=knots,
        control_points=[dict(point=p, weight=w) for p, w in zip(points, weights or [1.] * len(points))])


def request():
    operations = []
    def add(name, curves, angle):
        op = dict(op='loft', id='loft-refit-shallow-holdout-' + name, curves=curves,
            style='straight', closed=False, refit_tolerance=1e-7,
            inspect_inputs=True, sample_geometry=True, allow_empty=True)
        if angle is not None:
            op['refit_angle_tolerance'] = angle
        operations.append(op)
    for bend in [.099, .1, .100001, .101, .11]:
        a = math.radians(bend)
        points = [[0., 0.], [1., 0.], [1. + math.cos(a), math.sin(a)]]
        for i, angle in enumerate([None, 0., 1e-8, math.radians(.05), math.radians(.1), math.radians(.11)]):
            add('boundary-{}-{}'.format(bend, i), [curve(points, z) for z in [0., 2., 4.]], angle)
    for exponent in [10, 9, 8, 7, 6]:
        slope = 2. ** -exponent
        points = [[0., 0.], [1., 0.], [2., slope]]
        for name, degree, weights in [('linear', 1, None), ('quadratic', 2, None),
                                      ('rational', 1, [1., .5, 2.])]:
            add('dyadic-{}-{}'.format(exponent, name),
                [curve(points, z, degree, weights) for z in [0., 2., 4.]], 1e-8)
    slope = 1. / 256.
    a = [[0., 0.], [1., 0.], [3., 2. * slope]]
    b = [[0., 0.], [3., 0.], [4., slope]]
    add('unequal', [curve(a, 0.), curve(b, 2.)], 1e-8)
    add('line-companion', [curve(a, 0.), dict(degree=1, knots=[0., 0., 1., 1.],
        control_points=[dict(point=p, weight=1.) for p in [[0., 0., 2.], [4., .5, 2.]]])], 1e-8)
    spatial = copy.deepcopy(operations[-2]['curves'])
    for c in spatial:
        for cp in c['control_points']:
            x, y, z = cp['point']
            cp['point'] = [7. + 8. * z, -3. + 8. * x, 11. + 8. * y]
    add('spatial-unequal', spatial, 1e-8)
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_refit_shallow_holdouts', request(),
        ['generate_loft_refit_shallow_holdouts.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitShallowHoldouts20261026')
