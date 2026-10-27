"""Isolate cold numerical batch inverses from previous length-query history."""
import copy
from .capture_family import capture


def request():
    scenarios = [dict(id='cold-' + str(i), fractional_tolerance=a, history=[])
                 for i, a in enumerate([1e-6, 1e-5, 1e-4, 1e-3, .005, .01])]
    for index, accuracy in enumerate([1e-4, 1e-5]):
        for query in ['length', 'scalar', 'batch']:
            scenarios.append(dict(id=query + '-' + str(index),
                fractional_tolerance=accuracy,
                history=[dict(query=query, fractional_tolerance=1e-3)]))
        scenarios.append(dict(id='length-tight-' + str(index),
            fractional_tolerance=accuracy,
            history=[dict(query='length', fractional_tolerance=1e-6)]))
    operations = []
    for offset in [1e-6, 1e-5, 2e-4]:
        for reverse in [False, True]:
            ratio = 1. + offset
            points = [[0., 0., 0.], [1., 0., 0.], [1. + ratio, ratio * 1e-5, 0.]]
            if reverse:
                points.reverse()
            operations.append(dict(op='arc_length_batch',
                id='nurbs-batch-history-{:.0e}-{}'.format(offset, int(reverse)),
                curve=dict(degree=1, knots=[0., 0., 1., 2., 2.],
                    control_points=[dict(point=p, weight=1.) for p in points]),
                fractions=[i / 8. for i in range(9)],
                inspect_numerical_batch=copy.deepcopy(scenarios)))
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('nurbs_batch_history', request(), [
        'generate_nurbs_batch_history.py', 'arc_length_batch_probe.py',
        'geometry_validity_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleNurbsBatchHistory20261027')
