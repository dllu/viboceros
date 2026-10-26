"""Measure explicit corner angles independently of the legacy Loft overload."""
import copy
import math
from .capture_family import capture


def request():
    operations = []
    for degrees in [.03, .05, .08, .1, .15, .25, .5, .9, 1.1]:
        bend = math.radians(degrees)
        points = [[0., 0., 0.], [1., 0., 0.], [1. + math.cos(bend), math.sin(bend), 0.]]
        for factor in [0., .5, 1.5]:
            angle = 1e-8 if factor == 0. else bend * factor
            curves = []
            for z in [0., 2., 4.]:
                curves.append(dict(degree=1, knots=[0., 0., 1., 2., 2.],
                    control_points=[dict(point=[p[0], p[1], z], weight=1.) for p in points]))
            operations.append(dict(op='loft', id='loft-refit-shallow-angle-{}-{}'.format(degrees, factor),
                curves=curves, style='straight', closed=False, refit_tolerance=1e-7,
                refit_angle_tolerance=angle, inspect_inputs=True, sample_geometry=True, allow_empty=True))
    for degrees in [.05, .25, .9]:
        source = next(o for o in operations if o['id'] == 'loft-refit-shallow-angle-{}-0.0'.format(degrees))
        for variant in ['zero-angle', 'large-scale', 'quadratic']:
            op = copy.deepcopy(source)
            op['id'] += '-' + variant
            if variant == 'zero-angle':
                op['refit_angle_tolerance'] = 0.
            elif variant == 'large-scale':
                op['refit_tolerance'] *= 1000.
                for curve in op['curves']:
                    for cp in curve['control_points']:
                        cp['point'] = [x * 1000. for x in cp['point']]
            else:
                for curve in op['curves']:
                    a, b, c = [cp['point'] for cp in curve['control_points']]
                    midpoint = lambda p, q: [(x + y) * .5 for x, y in zip(p, q)]
                    curve['degree'] = 2
                    curve['knots'] = [0., 0., 0., 1., 1., 2., 2., 2.]
                    curve['control_points'] = [dict(point=p, weight=1.) for p in
                        [a, midpoint(a, b), b, midpoint(b, c), c]]
            operations.append(op)
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_refit_shallow_angles', request(),
        ['generate_loft_refit_shallow_angles.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitShallowAngles20261026')
