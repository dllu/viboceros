"""Bracket observed Ribbon plane and projection switches with owned curves."""
import copy
from .capture_family import capture
from .generate_ribbon import curve
from .generate_ribbon_cplane import request as base


def request():
    source = base()
    flat = next(o for o in source['operations'] if o['id'] == 'cubic')
    operations = []

    def add(name, **opts):
        op = copy.deepcopy(flat)
        op['id'] = name
        op.update(opts)
        operations.append(op)
        return op

    for epsilon in [.0062, .0063, .00632, .00633, .0064]:
        op = add('parallel-' + str(epsilon))
        for p in op['curve']['control_points']:
            p['point'][2] = p['point'][1] * epsilon
    for epsilon in [.0015, .002, .003, .004]:
        add('perpendicular-' + str(epsilon), curve=curve(
            [[0., 0., 0.], [1., epsilon, 1.], [4., 0., -1.], [5., 0., 2.]], 3))
    for scale in [.1, 10.]:
        for epsilon in [.001, .005]:
            add('perpendicular-scale-' + str(scale) + '-' + str(epsilon),
                curve=curve([[x*scale, y*scale, z*scale] for x,y,z in
                    [[0., 0., 0.], [1., epsilon, 1.], [4., 0., -1.], [5., 0., 2.]]], 3),
                distance=.15*scale, side=[0., 5.*scale, 0.])
    for height in [1e-5, .001, .005]:
        op = add('spatial-' + str(height))
        for p, z in zip(op['curve']['control_points'], [0., height, -height, 2.*height]):
            p['point'][2] = z
    source['operations'] = operations
    return source


if __name__ == '__main__':
    capture('ribbon_cplane_thresholds', request(), [
        'generate_ribbon_cplane_thresholds.py', 'generate_ribbon_cplane.py',
        'generate_ribbon_smooth.py', 'generate_ribbon.py', 'ribbon_probe.py', 'join_probe.py',
        'merge_edges_probe.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py'],
        'VibocerosOracleRibbonCPlaneThresholds20261109', timeout=1800)
