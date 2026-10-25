"""Isolate polynomial, parameter-domain and explicit-angle Loft Refit policy."""
import copy
import math
from .capture_family import capture
from .generate_loft_refit import request as original_request


def request():
    old = original_request()['operations']
    shapes = {}
    for name in ['line', 'quarter', 'cubic', 'circle']:
        shapes[name] = copy.deepcopy(next(o for o in old
            if o['id'] == 'loft-refit-{}-0.0-straight'.format(name))['curves'][0])

    def polynomial(degree, points, knots=None):
        return dict(degree=degree, knots=knots or [0.] * (degree + 1) + [1.] * (degree + 1),
                    control_points=[dict(point=p, weight=1.) for p in points])

    shapes.update(
        quadratic=polynomial(2, [[0., 0., 0.], [1., 3., 0.], [4., 0., 0.]]),
        polynomial_cubic=polynomial(3, [[0., 0., 0.], [1., 2., 0.], [3., -1., 0.], [4., 1., 0.]]),
        uneven_line=polynomial(3, [[0., 0., 0.], [.2, 0., 0.], [.7, 0., 0.], [3., 0., 0.]]),
        smooth_cubic=polynomial(3, [[0., 0., 0.], [.5, 2., 1.], [2., -1., 2.],
            [3., 1., 1.], [4., 0., 0.]], [0.] * 4 + [.3] + [1.] * 4),
        quintic=polynomial(5, [[float(i), y, 0.] for i, y in enumerate([0., 6., -13., 13., -6., 0.])]),
    )
    operations = []

    def add(name, source, tolerance, suffix='', **extra):
        other = copy.deepcopy(source)
        for cp in other['control_points']:
            cp['point'][2] += 2.
        operations.append(dict(op='loft', id='loft-refit-policy-{}-{}{}'.format(name, tolerance, suffix),
            curves=[copy.deepcopy(source), other], style='straight', closed=False,
            refit_tolerance=tolerance, inspect_inputs=True, sample_geometry=True,
            allow_empty=True, **extra))

    for name, source in shapes.items():
        for tolerance in [0., .1, .001, 1e-6]:
            add(name, source, tolerance)
    for name in ['quarter', 'circle', 'cubic']:
        for tolerance in [.001, 1e-6]:
            for index, angle in enumerate([0., 1e-6, math.pi / 180., math.pi / 2., math.pi]):
                add(name, shapes[name], tolerance, '-angle-' + str(index), refit_angle_tolerance=angle)
    for name in ['quadratic', 'polynomial_cubic', 'cubic']:
        for domain in [[-3., 7.], [0., 100.]]:
            source = copy.deepcopy(shapes[name])
            source['knots'] = [domain[0] + (domain[1] - domain[0]) * t for t in source['knots']]
            for tolerance in [.001, 1e-6]:
                add(name, source, tolerance, '-domain-' + str(domain[0]))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_refit_policy', request(), [
        'generate_loft_refit_policy.py', 'generate_loft_refit.py', 'generate_loft_rebuild.py',
        'fixtures/loft.json', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitPolicy20261025')
