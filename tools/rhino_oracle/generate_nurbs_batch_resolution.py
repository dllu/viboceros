"""Map natural-boundary choices over independent speed and tolerance scales."""
import copy
from .capture_family import capture
from .generate_nurbs_batch_history import request as original


def request():
    query = original()
    template = query['operations'][0]
    accuracies = [1e-8, 1e-7, 1e-6, 2e-6, 5e-6, 1e-5, 2e-5, 5e-5,
                  1e-4, 2e-4, 5e-4, .001, .002, .005, .01, .02]
    operations = []
    for index, offset in enumerate([0., 1e-10, 5e-8, 1e-7, 2e-7, 5e-7, 1e-6,
                                   2e-6, 5e-6, 1e-5, 2e-5, 5e-5, 1e-4, 2e-4,
                                   5e-4, 1e-3, .01]):
        for reverse in [False, True]:
            operation = copy.deepcopy(template)
            operation['id'] = 'nurbs-batch-resolution-{}-{}'.format(index, int(reverse))
            points = [[0., 0., 0.], [1., 0., 0.],
                      [2. + offset, (1. + offset) * 1e-5, 0.]]
            if reverse:
                points.reverse()
            for control, point in zip(operation['curve']['control_points'], points):
                control['point'] = point
            operation['inspect_numerical_batch'] = [
                dict(id='accuracy-' + str(i), fractional_tolerance=a, history=[])
                for i, a in enumerate(accuracies)]
            operations.append(operation)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('nurbs_batch_resolution', request(), [
        'generate_nurbs_batch_resolution.py', 'generate_nurbs_batch_history.py',
        'arc_length_batch_probe.py', 'geometry_validity_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleNurbsBatchResolution20261027')
