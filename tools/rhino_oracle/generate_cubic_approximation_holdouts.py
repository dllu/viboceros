"""Native positions away from the dyadic nodes used by cubic approximation."""
import copy
from .capture_family import capture
from .generate_cubic_approximation_sources import request as source_request

FRACTIONS = [.01369, .07231, .14759, .26873, .37123, .49387,
             .53191, .64817, .75913, .86447, .97361]


def request():
    query = copy.deepcopy(source_request())
    for operation in query['operations']:
        source = operation['curve']
        start = source['knots'][source['degree']]
        end = source['knots'][len(source['control_points'])]
        operation['id'] += '-holdout'
        operation['sided_parameters'] = [start + (end - start) * f for f in FRACTIONS]
    return query


if __name__ == '__main__':
    capture('cubic_approximation_holdouts', request(), [
        'generate_cubic_approximation_holdouts.py', 'generate_cubic_approximation_sources.py',
        'generate_loft_rebuild.py', 'fixtures/loft.json',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleCubicHoldouts20261025')
