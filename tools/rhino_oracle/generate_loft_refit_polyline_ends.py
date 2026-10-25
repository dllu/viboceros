"""Retain polygon point ends and smooth closed-profile seam locations."""
import copy
from .capture_family import capture
from .generate_loft_refit_polylines import request as profiles, polyline


def request():
    operations = []
    for op in profiles()['operations']:
        if not (any('-' + name + '-' in op['id'] for name in ['corner', 'rectangle'])
                and op['refit_tolerance'] == .001 and not op['closed']): continue
        for start, end in [(True, False), (False, True), (True, True)]:
            case = copy.deepcopy(op); case['id'] += '-ends-{}{}'.format(int(start), int(end))
            if start: case['start_point'] = [0., 0., -3.]
            if end: case['end_point'] = [0., 0., 9.]
            operations.append(case)
    for style in ['normal', 'loose', 'tight', 'straight', 'uniform']:
        curves = []
        for i in range(3):
            curves.append(polyline([[1., 0., 3. * i], [2. + i, 0., 3. * i],
                [2. + i, 1., 3. * i], [0., 1., 3. * i], [0., 0., 3. * i], [1., 0., 3. * i]]))
        operations.append(dict(op='loft', id='loft-refit-polyline-smooth-seam-' + style, curves=curves,
            style=style, closed=False, refit_tolerance=.001, inspect_inputs=True, sample_geometry=True, allow_empty=True))
    return dict(protocol_version=1, iterations=1, tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_refit_polyline_ends', request(), ['generate_loft_refit_polyline_ends.py',
        'generate_loft_refit_polylines.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitPolylineEnds20261025')
