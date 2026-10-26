"""Test whether source knot spacing changes paired adaptive fitting."""
import copy
import json
from .capture_family import HERE, capture


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_unequal_sampling.json').read_text())
    operations = []
    for reverse in [False, True]:
        for limit in [2.5e-7, 5e-7, 1e-6]:
            source = next(o for o in query['operations'] if o['id'] ==
                          'loft-refit-unequal-sampling-2.0-{}-{}'.format(int(reverse), limit))
            for clock in [.2, 1. / 3., .5, .8]:
                op = copy.deepcopy(source)
                op['id'] = 'loft-refit-adaptive-parameter-clock-{}-{}-{}'.format(int(reverse), limit, clock)
                for curve in op['curves']:
                    curve['knots'] = [0., 0., clock, 1., 1.]
                operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_adaptive_parameter_clocks', request(),
        ['generate_loft_refit_adaptive_parameter_clocks.py', 'fixtures/loft_refit_unequal_sampling.json',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitAdaptiveParameterClocks20261026')
