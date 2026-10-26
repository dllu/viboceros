"""Higher-degree region charts and smooth-profile explicit-angle holdouts."""
import copy
import json
import math
from .capture_family import HERE, capture


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_adjustment.json').read_text())
    operations = []
    for name in ['quarter', 'quadratic_2', 'polynomial_cubic']:
        original = next(o for o in query['operations']
                        if o['id'] == 'loft-refit-adjustment-{}-0.1'.format(name))
        for index, angle in enumerate([None, 0., 1e-6, math.pi / 6.,
                                       math.pi / 2., 5. * math.pi / 6., math.pi]):
            op = copy.deepcopy(original)
            op.pop('inspect_refit_adjustment', None)
            op['id'] = 'loft-refit-angle-smooth-{}-{}'.format(name, index)
            if angle is not None:
                op['refit_angle_tolerance'] = angle
            operations.append(op)
    for degree in [2, 3]:
        curves = []
        for i, (sx, sy) in enumerate([(1., 1.), (1.5, .5), (.75, 1.25)]):
            points = [[sx * j / degree, 0., 2. * i] for j in range(degree + 1)]
            points += [[sx, sy * j / degree, 2. * i] for j in range(1, degree + 1)]
            curves.append(dict(degree=degree, knots=[0.] * (degree + 1)
                               + [1.] * degree + [2.] * (degree + 1),
                               control_points=[dict(point=p, weight=1.) for p in points]))
        for domains in [False, True]:
            for index, angle in enumerate([None, 0., math.pi / 2.,
                                           5. * math.pi / 6., math.pi]):
                op = dict(op='loft', id='loft-refit-angle-chart-{}-{}-{}'.format(
                    degree, int(domains), index), curves=copy.deepcopy(curves),
                    style='straight', closed=False, refit_tolerance=0.,
                    inspect_inputs=True, sample_geometry=True, allow_empty=True)
                if angle is not None:
                    op['refit_angle_tolerance'] = angle
                if domains:
                    for i, curve in enumerate(op['curves']):
                        curve['knots'] = [-3. * i + (i + 1.) * t for t in curve['knots']]
                operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_angle_holdouts', request(), [
        'generate_loft_refit_angle_holdouts.py', 'fixtures/loft_refit_adjustment.json',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitAngleHoldouts20261026')
