"""Bounded multiplicity ranges, rational curves and fitted-piece joins."""
import copy
import json
from .capture_family import HERE, capture


def polynomial(points, knots):
    return dict(degree=3, knots=knots,
                control_points=[dict(point=p, weight=1.) for p in points])


def request():
    query = json.loads((HERE / 'fixtures/remove_multi_knot.json').read_text())
    curves = []
    for key, name in [('cubic-exclude-full', 'polynomial'),
                      ('linear-remove-interior', 'linear'),
                      ('rational-cubic-exclude-full', 'rational')]:
        curves.append((name, copy.deepcopy(next(o['curve'] for o in query['operations'] if o['id'] == key))))
    curves.append(('redundant-line', polynomial([[x, 0., 0.] for x in [0., .5, 1., 2., 2.5, 3.]],
                   [0.] * 4 + [.5] * 2 + [1.] * 4)))
    native = json.loads((HERE / 'observations/loft_refit_subdivision.json').read_text())
    rows = {row['id']: row['value'] for row in native['results']}
    prefix = 'loft-refit-subdivision-quarter-'
    halves = []
    for side in ['left', 'right']:
        s = rows[prefix + side + '-unit-domain-0.001']['surfaces'][0]
        halves.append(copy.deepcopy(s['control_points'][::s['control_count'][0]]))
    halves[1][0] = copy.deepcopy(halves[0][-1])
    curves.append(('fresh-half-join', dict(degree=3, knots=[0.] * 4 + [.5] * 3 + [1.] * 4,
                   control_points=halves[0] + halves[1][1:])))
    s = rows[prefix + 'full-0.001']['surfaces'][0]
    cp = copy.deepcopy(s['control_points'][::s['control_count'][0]])
    middle = dict(point=[(a + b) * .5 for a, b in zip(cp[2]['point'], cp[3]['point'])], weight=1.)
    curves.append(('refined-full-fit', dict(degree=3, knots=[0.] * 4 + [.5] * 3 + [1.] * 4,
                   control_points=cp[:3] + [middle] + cp[3:])))
    operations = []
    for name, curve in curves:
        degree = curve['degree']
        ranges = [(0, degree + 1), (1, degree + 1), (2, degree + 1), (0, degree), (1, degree)]
        for minimum, maximum in ranges:
            if minimum >= maximum:
                continue
            for limit in [0., 1e-9, 1e-5, .1, 10.]:
                operations.append(dict(op='curve_knot_reduction',
                    id='knot-reduction-{}-{}-{}-{}'.format(name, minimum, maximum, limit),
                    curve=copy.deepcopy(curve), minimum_multiplicity=minimum,
                    maximum_multiplicity=maximum, absolute_tolerance=limit))
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('knot_reduction', request(), ['generate_knot_reduction.py',
        'fixtures/remove_multi_knot.json', 'observations/loft_refit_subdivision.json',
        'knot_reduction_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleKnotReduction20261025')
