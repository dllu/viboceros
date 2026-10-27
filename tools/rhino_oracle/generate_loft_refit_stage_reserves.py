"""Independent spacing/tolerance holdouts for complete refit-stage reserves."""
import copy
import json
from .capture_family import HERE, capture


def request():
    query = json.loads((HERE / 'fixtures/loft_refit_unequal_sampling.json').read_text())
    template = query['operations'][0]
    operations = []
    for ratio in [.4, .6, 1.6, 2.4]:
        for reverse in [False, True]:
            for tolerance in [2e-7, 4e-7, 8e-7]:
                operation = copy.deepcopy(template)
                operation['id'] = 'loft-refit-stage-reserve-{}-{}-{}'.format(
                    ratio, int(reverse), tolerance)
                operation['refit_tolerance'] = tolerance
                for curve in operation['curves']:
                    height = curve['control_points'][0]['point'][2]
                    points = [[0., 0., height], [1., 0., height],
                              [1. + ratio, ratio * 1e-5, height]]
                    if reverse:
                        points.reverse()
                    for control, point in zip(curve['control_points'], points):
                        control['point'] = point
                operations.append(operation)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_stage_reserves', request(), [
        'generate_loft_refit_stage_reserves.py', 'fixtures/loft_refit_unequal_sampling.json',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitStageReserves20261027')
