"""Raw and simplified public fitting on fine circular and elliptical profiles."""
import copy
import json
from .capture_family import HERE, capture


def request():
    quarter = json.loads((HERE / 'fixtures/loft_refit.json').read_text())
    original = next(op for op in quarter['operations'] if op['id'] == 'loft-refit-quarter-1e-06-normal')
    operations = []
    for name in ['heterogeneous', 'circle']:
        for tolerance in [.0007, 1e-6]:
            op = copy.deepcopy(original)
            if name == 'circle':
                source = copy.deepcopy(op['curves'][0])
                op['curves'] = []
                for height in [0., 2., 4.]:
                    curve = copy.deepcopy(source)
                    for p in curve['control_points']:
                        p['point'][2] += height
                    op['curves'].append(curve)
            op.update(id='loft-refit-arc-clocks-{}-{}'.format(name, tolerance),
                      refit_tolerance=tolerance, inspect_inputs=True,
                      inspect_refit_sources=True, sample_geometry=True)
            operations.append(op)
    return dict(protocol_version=1, iterations=1, operations=operations)


if __name__ == '__main__':
    capture('loft_refit_arc_clocks', request(), ['generate_loft_refit_arc_clocks.py',
        'fixtures/loft_refit.json', 'loft_refit_source_probe.py', 'loft_refit_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitArcClocks20261027')
