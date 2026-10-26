"""Separate fitting trimmed profiles from joining a subdivided full profile."""
import copy
import json
from .capture_family import HERE, capture


def split(source, parameter, preserve_domain):
    degree = source['degree']
    net = [[x * cp['weight'] for x in cp['point']] + [cp['weight']]
           for cp in source['control_points']]
    if len(net) != degree + 1:
        raise ValueError('subdivision reference needs a single Bezier source')
    left, right = [net[0]], [net[-1]]
    for _ in range(degree):
        net = [[(1. - parameter) * a + parameter * b for a, b in zip(p, q)]
               for p, q in zip(net, net[1:])]
        left.append(net[0])
        right.append(net[-1])

    def curve(points, interval):
        a, b = interval if preserve_domain else (0., 1.)
        return dict(degree=degree, knots=[a] * (degree + 1) + [b] * (degree + 1),
                    control_points=[dict(point=[x / p[3] for x in p[:3]], weight=p[3])
                                    for p in points])

    a, b = source['knots'][degree], source['knots'][-degree - 1]
    middle = (1. - parameter) * a + parameter * b
    return curve(left, (a, middle)), curve(right[::-1], (middle, b))


def midpoint(surface):
    nu, nv = surface['control_count']
    controls = surface['control_points'][::nu]
    if nv != 4 or any(cp['weight'] != 1. for cp in controls):
        raise ValueError('expected a polynomial cubic midpoint reference')
    return [sum(weight * cp['point'][axis] for weight, cp in
                zip([.125, .375, .375, .125], controls)) for axis in range(3)]


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_coarse.json').read_text())
    coarse = json.loads((HERE / 'observations/loft_refit_coarse.json').read_text())
    fitting = json.loads((HERE / 'observations/loft_refit_adjustment_fit_clock.json').read_text())
    operations = []
    for name in ['quarter', 'quadratic', 'polynomial_cubic']:
        original = next(o for o in query['operations'] if o['id'] == 'loft-refit-coarse-' + name + '-0.1')
        source = original['curves'][0]
        variants = [('full', source)]
        for preserve in [False, True]:
            left, right = split(source, .5, preserve)
            suffix = '-source-domain' if preserve else '-unit-domain'
            variants += [('left' + suffix, left), ('right' + suffix, right)]
        reversed_source = copy.deepcopy(source)
        reversed_source['control_points'].reverse()
        variants.append(('reversed', reversed_source))
        for variant, curve in variants:
            for tolerance in [.001, .01]:
                other = copy.deepcopy(curve)
                for cp in other['control_points']:
                    cp['point'][2] += 2.
                operations.append(dict(op='loft', id='loft-refit-subdivision-{}-{}-{}'.format(name, variant, tolerance),
                    curves=[copy.deepcopy(curve), other], style='straight', closed=False,
                    refit_tolerance=tolerance, inspect_inputs=True, sample_geometry=True,
                    inspect_refit_adjustment=True, refit_adjustment_fractional_tolerance=1e-2,
                    allow_empty=True))
        initial = next(r['value']['surfaces'][0] for r in coarse['results']
                       if r['id'] == 'loft-refit-coarse-' + name + '-1.0')
        if name == 'polynomial_cubic':
            corrected = next(r['value']['surfaces'][0] for r in coarse['results']
                             if r['id'] == 'loft-refit-coarse-' + name + '-0.1')
        else:
            corrected = next(r['value']['surfaces'][0] for r in fitting['results']
                             if r['id'] == 'loft-refit-fit-clock-' + name + '-0.03-rational')
        chord = [(a + b) * .5 for a, b in zip(source['control_points'][0]['point'],
                                             source['control_points'][-1]['point'])]
        for label, target in [('initial', midpoint(initial)), ('corrected', midpoint(corrected)), ('chord', chord)]:
            operations.append(dict(op='nurbs_curve_closest_point',
                id='loft-refit-subdivision-query-' + name + '-' + label,
                target=target, **copy.deepcopy(source)))
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_subdivision', request(), ['generate_loft_refit_subdivision.py',
        'fixtures/loft_refit_coarse.json', 'observations/loft_refit_coarse.json',
        'observations/loft_refit_adjustment_fit_clock.json', 'loft_refit_adjustment_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitSubdivision20261025')
