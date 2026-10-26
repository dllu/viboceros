"""Independent scalar inverse lengths before general smooth Loft fitting."""
import copy
from .capture_family import capture
from .generate_loft_refit_clock import request as clocks
from .generate_loft_refit_line_inputs import line
from .generate_loft_refit_polylines import request as polygons


def request():
    operations = []
    for old in clocks()['operations']:
        if old['refit_tolerance'] != .1: continue
        for shifted in [False, True]:
            op = copy.deepcopy(old)
            op.pop('inspect_refit_clock'); op['refit_tolerance'] = 0.
            op['inspect_scalar_clock'] = True
            op['id'] = op['id'].replace('loft-refit-clock-', 'loft-scalar-clock-') + ('-shifted' if shifted else '')
            if shifted:
                for c in op['curves']: c['knots'] = [-3. + 10. * t for t in c['knots']]
            operations.append(op)
    for name, source in [('rational_line', line(1, [0., 1.], [1., 2.])),
            ('uneven_line', line(3, [0., .0625, .25, 1.])),
            ('unclamped_line', line(2, [0., .25, 1.], [1., 2., .5], [-1., -1., 0., 1., 2., 2.]))]:
        curves = [copy.deepcopy(source) for _ in range(2)]
        for c in curves[1]['control_points']: c['point'][2] += 2.
        operations.append(dict(op='loft', id='loft-scalar-clock-' + name, curves=curves, style='straight',
            refit_tolerance=0., inspect_inputs=True, inspect_scalar_clock=True, allow_empty=True))
    for name in ['rational', 'constant_span']:
        op = next(copy.deepcopy(o) for o in polygons()['operations'] if o['id']=='loft-refit-polyline-' + name + '-straight-0.001')
        op['id'] = 'loft-scalar-clock-' + name
        op['refit_tolerance'] = 0.; op['inspect_scalar_clock'] = True; operations.append(op)
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_scalar_clock', request(), ['generate_loft_scalar_clock.py',
        'generate_loft_refit_clock.py', 'generate_loft_refit_policy.py', 'generate_loft_refit.py',
        'generate_loft_rebuild.py', 'generate_loft_refit_line_inputs.py', 'generate_loft_refit_polylines.py',
        'fixtures/loft.json', 'loft_scalar_clock_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftScalarClock20261025')
