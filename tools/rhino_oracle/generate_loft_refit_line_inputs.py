"""Mixed straight-profile clocks and unclamped active domains for Loft Refit."""
import copy
from .capture_family import capture


def line(degree, fractions, weights=None, knots=None, z=0., scale=1.):
    return dict(degree=degree, knots=knots or [0.] * (degree + 1) + [1.] * (degree + 1),
        control_points=[dict(point=[4. * t * scale, 2. * t * scale, z],
            weight=(weights or [1.] * len(fractions))[i]) for i, t in enumerate(fractions)])


def request():
    plain = line(1, [0., 1.])
    uneven = line(3, [0., .0625, .25, 1.])
    rational = line(2, [0., .25, 1.], [1., 2., .5])
    unclamped = line(2, [0., .25, 1.], knots=[-1., -1., 0., 1., 2., 2.])
    unclamped_rational = line(2, [0., .25, 1.], [1., 2., .5], [-1., -1., 0., 1., 2., 2.])
    unclamped_cubic = line(3, [0., .125, .5, .75, 1.],
        knots=[-2., -2., -1., 0., .25, 1., 2., 3., 3.])
    contexts = dict(unclamped=[unclamped] * 3, unclamped_rational=[unclamped_rational] * 3,
        unclamped_cubic=[unclamped_cubic] * 3, mixed_clocks=[plain, uneven, rational],
        mixed_active=[unclamped, plain, unclamped_rational],
        reversed_shifted=[unclamped_cubic, rational, uneven])
    operations = []
    for name, templates in contexts.items():
        curves = [copy.deepcopy(template) for template in templates]
        for i, curve in enumerate(curves):
            for cp in curve['control_points']:
                cp['point'][0] *= i + 1
                cp['point'][1] *= i + 1
                cp['point'][2] += 2. * i
            if name == 'reversed_shifted':
                curve['knots'] = [100. + 5. * k for k in curve['knots']]
                if i == 1:
                    curve['control_points'].reverse()
        for style in ['normal', 'loose', 'tight', 'straight', 'uniform']:
            for tolerance in [.1, .001, 1e-6]:
                operations.append(dict(op='loft', id='loft-refit-input-{}-{}-{}'.format(name, style, tolerance),
                    curves=copy.deepcopy(curves), style=style, closed=False,
                    refit_tolerance=tolerance, inspect_inputs=True, sample_geometry=True, allow_empty=True))
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_refit_line_inputs', request(), ['generate_loft_refit_line_inputs.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitLineInputs20261025')
