"""Single-profile direction choices on both halves of an open rail."""
from copy import deepcopy
from .capture_family import capture
from .generate_sweep1_automatic_curved import request as curved


def request():
    query = curved()
    spatial = deepcopy(query['operations'][4]['rail'])
    operations = []
    def add(name, rail, start, end):
        operations.append(dict(op='sweep1', id=name, rail=rail,
                               sections=[dict(type='line', start=start, end=end)],
                               command=True, refit_rail=False, inspect_definition=True))
    for z in [1., 2.5, 4.]:
        add('line-height-' + str(z), dict(type='line', start=[0., 0., 0.], end=[0., 0., 5.]),
            [0., 0., z], [2., 0., z])
    controls = [p['point'] for p in spatial['control_points']]
    for t in [.49, .51, .55, .75]:
        coefficients = [(1-t)**3, 3*t*(1-t)**2, 3*t*t*(1-t), t**3]
        p = [sum(c*point[i] for c, point in zip(coefficients, controls)) for i in range(3)]
        add('spatial-direction-' + str(t), spatial, p, [p[0], p[1], p[2]+1.])
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('sweep1_automatic_direction', request(),
            ['generate_sweep1_automatic_direction.py', 'generate_sweep1_automatic_curved.py',
             'rhino_worker.py', 'client.py', 'structural_sdk.py'],
            'VibocerosOracleSweep1AutomaticDirection20261027')
