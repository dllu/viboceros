"""Isolate adaptive joining as transverse curvature crosses model-unit limits."""
import copy
from .capture_family import capture
from .generate_loft_refit_adaptive_shapes import request as shapes


def request():
    query = shapes()
    operations = []
    for source in query['operations'][::3]:
        for scale in [1., .25, .0625, .015625]:
            op = copy.deepcopy(source)
            op['id'] = op['id'].replace('adaptive-shapes-', 'join-modes-') + '-' + str(scale)
            op['refit_tolerance'] *= scale
            for i, curve in enumerate(op['curves']):
                for control in curve['control_points']:
                    control['point'][1] *= scale
                    control['point'][2] = 2. * i + (control['point'][2] - 2. * i) * scale
            op.update(inspect_inputs=True, inspect_refit_sources=True, sample_geometry=True)
            operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_join_modes', request(), [
        'generate_loft_refit_join_modes.py', 'generate_loft_refit_adaptive_shapes.py',
        'loft_refit_source_probe.py', 'loft_refit_probe.py', 'rhino_worker.py',
        'client.py', 'structural_sdk.py'], 'VibocerosOracleLoftRefitJoinModes20261027')
