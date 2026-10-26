"""Unresolved bends, rational region clocks and G2 boundary holdouts."""
import copy
import math
from .capture_family import capture


def request():
    sources = {}
    for degree in [1, 2, 3]:
        for rational in [False, True]:
            if degree == 1 and rational:
                continue
            points = [[j / degree, 0., 0.] for j in range(degree + 1)]
            points += [[1. + j / degree, 1e-5 * j / degree, 0.]
                       for j in range(1, degree + 1)]
            weights = [2. if rational and j % degree else 1.
                       for j in range(len(points))]
            sources['bend-{}-{}'.format(degree, int(rational))] = dict(
                degree=degree, knots=[0.] * (degree + 1) + [1.] * degree
                + [2.] * (degree + 1), control_points=[
                    dict(point=p, weight=w) for p, w in zip(points, weights)])
    sources['smooth-split'] = dict(degree=2,
        knots=[0., 0., 0., .5, .5, 1., 1., 1.], control_points=[
            dict(point=[x, y, 0.], weight=1.) for x, y in [
                (0., 0.), (.5, 5e-6), (1., 5e-6), (1.5, 5e-6), (2., 0.)]])
    sources['curvature-jump'] = dict(degree=3,
        knots=[0.] * 4 + [.5] * 2 + [1.] * 4, control_points=[
            dict(point=[x, y, 0.], weight=1.) for x, y in [
                (0., 0.), (.25, 0.), (.75, 0.), (1.25, 1e-5),
                (1.75, 2e-5), (2., 2e-5)]])
    for degrees in [.05, .1, .25, .5]:
        angle = math.radians(degrees)
        sources['angle-' + str(degrees)] = dict(degree=1,
            knots=[0., 0., 1., 2., 2.], control_points=[
                dict(point=p, weight=1.) for p in [[0., 0., 0.], [1., 0., 0.],
                    [1. + math.cos(angle), math.sin(angle), 0.]]])
    operations = []
    for name, source in sources.items():
        for variant in ['plain', 'domain', 'frame']:
            curves = []
            for i in range(3):
                curve = copy.deepcopy(source)
                for cp in curve['control_points']:
                    x, y, z = cp['point']
                    point = [x, y, z + 2. * i]
                    cp['point'] = [7. + point[2], -3. + point[0], 5. + point[1]] \
                        if variant == 'frame' else point
                if variant == 'domain':
                    curve['knots'] = [-3. * i + (i + 1.) * t for t in curve['knots']]
                curves.append(curve)
            for tolerance in [.001, 1e-6]:
                operations.append(dict(op='loft', id='loft-refit-multispan-{}-{}-{}'.format(
                    name, variant, tolerance), curves=copy.deepcopy(curves),
                    style='straight', closed=False, refit_tolerance=tolerance,
                    inspect_inputs=True, sample_geometry=True, allow_empty=True))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8),
                operations=operations)


if __name__ == '__main__':
    capture('loft_refit_multispan', request(), ['generate_loft_refit_multispan.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitMultispan20261026')
