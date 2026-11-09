"""Coarse-fit holdouts on rational and nonuniform fixed-plane sources."""
import copy
from .capture_family import capture
from .generate_ribbon_cplane import request as base


def request():
    query = base()
    operations = []
    for name in ['cubic-tilted', 'rational-tilted', 'nonuniform-tilted']:
        source = next(o for o in query['operations'] if o['id'] == name)
        for tolerance in [.02, .1]:
            op = copy.deepcopy(source)
            op['id'] = name + '-fit-' + str(tolerance)
            op['fit_tolerance'] = tolerance
            operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('ribbon_cplane_holdouts', request(), [
        'generate_ribbon_cplane_holdouts.py', 'generate_ribbon_cplane.py',
        'generate_ribbon_smooth.py', 'generate_ribbon.py', 'ribbon_probe.py', 'join_probe.py',
        'merge_edges_probe.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py'],
        'VibocerosOracleRibbonCPlaneHoldouts20261109', timeout=1800)
