"""Public closest-point queries at the scalar length stations used for fitting."""
import copy
import json
from .capture_family import HERE, capture


def request():
    initial = json.loads((HERE / 'observations/loft_refit_coarse.json').read_text())
    clocks = json.loads((HERE / 'observations/loft_scalar_clock.json').read_text())
    operations = []
    for name in ['quarter', 'quadratic', 'polynomial_cubic']:
        surface = next(r['value']['surfaces'][0] for r in initial['results']
            if r['id'] == 'loft-refit-coarse-' + name + '-1.0')
        nu, nv = surface['control_count']
        if nv != 4: raise ValueError('reference is not one initial cubic')
        controls = copy.deepcopy(surface['control_points'][::nu])
        rows = next(r['value']['scalar_clock'] for r in clocks['results']
            if r['id'] == 'loft-scalar-clock-' + name + '-0.1')
        for accuracy in [1e-8, 1e-4, 1e-3]:
            row = next(r for r in rows if r['fractional_tolerance'] == accuracy)
            point = next(s['point'] for s in row['forward'] if s['fraction'] == .5)
            for domain in [1., surface['knots_v'][-1]]:
                operations.append(dict(op='nurbs_curve_closest_point',
                    id='loft-refit-adjustment-query-{}-{}-{}'.format(name, accuracy, domain),
                    degree=3, knots=[0.] * 4 + [domain] * 4,
                    control_points=copy.deepcopy(controls), target=point))
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_refit_adjustment_queries', request(), ['generate_loft_refit_adjustment_queries.py',
        'observations/loft_refit_coarse.json', 'observations/loft_scalar_clock.json',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitAdjustmentQueries20261025')
