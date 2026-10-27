"""Check stopping-error boundaries and overlapping endpoint acceptance ranges."""
import copy
import math
from .capture_family import capture
from .generate_scalar_checkpoint_acceptance import request as checkpoints


def request():
    query = checkpoints()
    operations = []
    for source in query['operations']:
        limits = [source['fractional_tolerance']]
        if source['id'] == 'scalar-checkpoint-rational-plain-0.02':
            limits += [.25, .5, .75]
        for accuracy in limits:
            op = copy.deepcopy(source)
            op['id'] = source['id'].replace('scalar-checkpoint-', 'scalar-endpoint-boundary-')
            if accuracy != source['fractional_tolerance']:
                op['id'] += '-' + str(accuracy)
            op['fractional_tolerance'] = accuracy
            fractions = sorted({0., 1., .5, *[neighbor for f in [accuracy, 1. - accuracy]
                for neighbor in [math.nextafter(f, 0.), f, math.nextafter(f, 1.)]]})
            op['fractions'] = fractions + list(reversed(fractions))
            operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('scalar_endpoint_boundaries', request(),
        ['generate_scalar_endpoint_boundaries.py', 'generate_scalar_checkpoint_acceptance.py',
         'fixtures/loft_scalar_clock.json', 'loft_scalar_clock_probe.py',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleScalarEndpointBoundaries20261026')
