"""Distinguish automatic full-rail anchoring from partial-rail placement."""
from copy import deepcopy
from math import cos, pi, sin, sqrt
from .capture_family import capture
from .generate_sweep1_automatic_curved import request as curved


def request():
    query = curved()
    arc = deepcopy(query['operations'][0]['rail'])
    spatial = deepcopy(query['operations'][4]['rail'])
    operations = []
    def add(name, rail, start, end):
        operations.append(dict(op='sweep1', id=name, rail=rail,
                               sections=[dict(type='line', start=start, end=end)],
                               command=True, refit_rail=False, inspect_definition=True))
    add('arc-thirty', arc, [3*cos(pi/6), 3*sin(pi/6), 0.], [4*cos(pi/6), 4*sin(pi/6), 0.])
    arc_nurbs = dict(type='nurbs', degree=2,
                     control_points=[dict(point=p, weight=w) for p, w in
                                     zip([[3., 0., 0.], [3., 3., 0.], [0., 3., 0.]], [1., sqrt(.5), 1.])],
                     knots=[0.]*3 + [3*pi/2]*3)
    add('rational-arc-middle', arc_nurbs, [3/sqrt(2), 3/sqrt(2), 0.], [4/sqrt(2), 4/sqrt(2), 0.])
    add('nurbs-line-interior', dict(type='nurbs', degree=1,
                                   control_points=[dict(point=p, weight=1.) for p in [[0., 0., 0.], [0., 0., 5.]]],
                                   knots=[0., 0., 5., 5.]), [0., 0., 2.], [2., 0., 2.])
    controls = [p['point'] for p in spatial['control_points']]
    for t in [1/3, 2/3, .25]:
        coefficients = [(1-t)**3, 3*t*(1-t)**2, 3*t*t*(1-t), t**3]
        p = [sum(c*point[i] for c, point in zip(coefficients, controls)) for i in range(3)]
        add('spatial-station-' + str(t), spatial, p, [p[0], p[1], p[2]+1.])
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('sweep1_automatic_stations', request(),
            ['generate_sweep1_automatic_stations.py', 'generate_sweep1_automatic_curved.py',
             'rhino_worker.py', 'client.py', 'structural_sdk.py'],
            'VibocerosOracleSweep1AutomaticStations20261027')
