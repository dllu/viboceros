"""Separate derivative continuity, source degree and affine chart effects."""
import copy
import json
from .capture_family import HERE, capture


def elevated(curve, degree):
    points = [c['point'] for c in curve['control_points']]
    controls = []
    for segment, (a, b) in enumerate(zip(points, points[1:])):
        for j in range(int(segment != 0), degree + 1):
            fraction = j / float(degree)
            point = [a[i] + fraction * (b[i] - a[i]) for i in range(3)]
            controls.append(dict(point=point, weight=1.))
    clock = curve['knots'][2]
    return dict(degree=degree, control_points=controls,
        knots=[0.] * (degree + 1) + [clock] * degree + [1.] * (degree + 1))


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_adaptive_parameter_clocks.json').read_text())
    selected = [op for op in query['operations'] if op['refit_tolerance'] in [2.5e-7, 1e-6]]
    operations = []
    for original in selected:
        for variant in ['plain', 'shifted', 'cubic', 'quintic']:
            op = copy.deepcopy(original)
            op['id'] = original['id'].replace('loft-refit-adaptive-parameter-clock-',
                'loft-refit-source-continuity-') + '-' + variant
            op['inspect_refit_sources'] = True
            if variant == 'shifted':
                for curve in op['curves']:
                    curve['knots'] = [100. + 7. * k for k in curve['knots']]
            elif variant in ['cubic', 'quintic']:
                op['curves'] = [elevated(c, 3 if variant == 'cubic' else 5) for c in op['curves']]
            operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_source_continuity', request(),
        ['generate_loft_refit_source_continuity.py', 'fixtures/loft_refit_adaptive_parameter_clocks.json',
         'loft_refit_source_probe.py', 'loft_refit_probe.py',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitSourceContinuity20261026')
