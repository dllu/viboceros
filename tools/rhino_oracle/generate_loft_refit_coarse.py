"""Separate the cubic proposal from tolerance-driven subdivision in Loft Refit."""
import copy
from .capture_family import capture
from .generate_loft_refit_policy import request as policy


def request():
    query = policy()
    shapes = {}
    for op in query['operations']:
        if op['refit_tolerance'] == .1 and not any(x in op['id'] for x in ['-angle-', '-domain-']):
            name = op['id'].split('loft-refit-policy-', 1)[1].rsplit('-', 1)[0]
            if name not in ['line', 'uneven_line']:
                shapes[name] = copy.deepcopy(op['curves'][0])
    for index, point in enumerate([[0., 3., 0.], [2., 3., 0.], [1., 1., 0.]]):
        c = copy.deepcopy(shapes['quadratic'])
        c['control_points'][1]['point'] = point
        shapes['quadratic_' + str(index)] = c
    for index, delta in enumerate([[0., 1., 0.], [1., 0., 0.], [0., 0., 1.]]):
        c = copy.deepcopy(shapes['polynomial_cubic'])
        c['control_points'][1]['point'] = [a + b for a, b in zip(c['control_points'][1]['point'], delta)]
        shapes['polynomial_cubic_' + str(index)] = c
    operations = []
    for name, source in shapes.items():
        for tolerance in [.1, 1., 100.]:
            other = copy.deepcopy(source)
            for cp in other['control_points']:
                cp['point'][2] += 2.
            operations.append(dict(op='loft', id='loft-refit-coarse-{}-{}'.format(name, tolerance),
                curves=[copy.deepcopy(source), other], style='straight', closed=False,
                refit_tolerance=tolerance, inspect_inputs=True, sample_geometry=True, allow_empty=True))
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_coarse', request(), ['generate_loft_refit_coarse.py',
        'generate_loft_refit_policy.py', 'generate_loft_refit.py', 'generate_loft_rebuild.py',
        'fixtures/loft.json', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitCoarse20261025')
