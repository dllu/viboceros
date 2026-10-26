"""Check whether inherited source clocks change local cubic acceptance."""
import copy
import json
from .capture_family import HERE, capture


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_interval_sampling.json').read_text())
    operations = []
    for length in [.75, .375]:
        source = next(o for o in query['operations'] if o['id'] == 'loft-refit-interval-sampling-{}-0-0.0'.format(length))
        for clock in [.2, 1. / 3., .5, 2. / 3., .8]:
            for shifted in [False, True]:
                op = copy.deepcopy(source)
                op['id'] = 'loft-refit-interval-clock-{}-{}-{}'.format(length, clock, int(shifted))
                for curve in op['curves']:
                    curve['knots'] = [0., 0., clock, 1., 1.]
                    if shifted:
                        curve['knots'] = [100. + 7. * k for k in curve['knots']]
                operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_interval_clocks', request(),
        ['generate_loft_refit_interval_clocks.py', 'fixtures/loft_refit_interval_sampling.json',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitIntervalClocks20261026')
