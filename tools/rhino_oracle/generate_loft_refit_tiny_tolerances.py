"""Check the SDK fitting floor against disabled fitting on several source types."""
import copy
import json
from .capture_family import HERE, capture


def request():
    base = json.loads((HERE / 'fixtures/loft_refit_adaptive_lines.json').read_text())
    bend = next(o['curves'] for o in base['operations'] if o['id'] == 'loft-refit-adaptive-angle-0.05-0.001-1.0')
    source = json.loads((HERE / 'fixtures/loft_refit_candidates.json').read_text())
    quarter = next(o['curves'] for o in source['operations'] if o['id'] == 'loft-refit-candidate-quarter-straight')
    line = [dict(degree=3, knots=[0.] * 4 + [1.] * 4,
        control_points=[dict(point=[x, 0., z], weight=1.) for x in [0., .2, .7, 3.]]) for z in [0., 2., 4.]]
    epsilon = 2. ** -26
    operations = []
    for name, curves in [('bend', bend), ('quarter', quarter), ('line', line)]:
        for limit in [0., 1e-10, 1e-9, 1e-8, epsilon * (1. - 1e-6), epsilon, epsilon * (1. + 1e-6), 1e-7]:
            operations.append(dict(op='loft', id='loft-refit-tiny-tolerance-{}-{}'.format(name, limit),
                curves=copy.deepcopy(curves), style='straight', closed=False, refit_tolerance=limit,
                inspect_inputs=True, sample_geometry=True, allow_empty=True))
    base['operations'] = operations
    return base


if __name__ == '__main__':
    capture('loft_refit_tiny_tolerances', request(),
        ['generate_loft_refit_tiny_tolerances.py', 'fixtures/loft_refit_adaptive_lines.json',
         'fixtures/loft_refit_candidates.json', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitTinyTolerances20261026')
