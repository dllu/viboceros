"""Separate source clocks from the remaining adaptive construction gaps."""
import copy
from .capture_family import capture
from .generate_loft_refit_adaptive_shapes import request as shapes


def request():
    query = shapes()
    operations = []
    for source in query['operations'][::3]:
        op = copy.deepcopy(source)
        op['id'] = op['id'].replace('adaptive-shapes-', 'adaptive-shape-clocks-')
        op.update(inspect_inputs=True, inspect_refit_sources=True, sample_geometry=True)
        operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('loft_refit_adaptive_shape_clocks', request(), [
        'generate_loft_refit_adaptive_shape_clocks.py',
        'generate_loft_refit_adaptive_shapes.py', 'loft_refit_source_probe.py',
        'loft_refit_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitAdaptiveShapeClocks20261027')
