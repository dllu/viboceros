"""Separate section-count, scale, source-domain and representation policies."""
import copy
from .capture_family import capture
from .generate_sweep1_refit import request as base_request


def request():
    root = base_request()
    template = next(op for op in root['operations'] if op['id'] == 'open-refit-0.01')
    operations = []
    def add(name, curves, fit):
        op = copy.deepcopy(template)
        op.update(id=name, sections=curves, refit_tolerance=fit,
                  parameters=[0.] if len(curves) == 1 else [0., 5.])
        operations.append(op)
    original = template['sections'][0]
    for scale in [.5, 1., 2., 10.]:
        source = copy.deepcopy(original)
        for cp in source['control_points']:
            cp['point'] = [c * scale for c in cp['point']]
        for fit in [.001, .01]:
            add('single-scale-{}-{}'.format(scale, fit), [source], fit)
            second = copy.deepcopy(source)
            for cp in second['control_points']:
                cp['point'][2] = 5.
            add('equal-pair-scale-{}-{}'.format(scale, fit), [source, second], fit)
    variants = {}
    source = copy.deepcopy(original)
    source['knots'] = [20.] * 3 + [100.] * 3
    variants['domain'] = source
    source = copy.deepcopy(original)
    source['degree'] = 3
    source['knots'] = [0.] * 4 + [1.] * 4
    source['control_points'] = [dict(point=p, weight=1.) for p in
        [[0., 0., 0.], [2./3., 4./3., 0.], [5./3., 4./3., 0.], [3., 0., 0.]]]
    variants['elevated'] = source
    source = copy.deepcopy(original)
    source['control_points'].reverse()
    variants['reversed'] = source
    source = copy.deepcopy(original)
    for cp in source['control_points']:
        cp['point'] = [cp['point'][0] + 100., cp['point'][1] - 50., 0.]
    variants['translated'] = source
    for name, source in variants.items():
        for fit in [.001, .01]:
            add(name + '-' + str(fit), [source], fit)
    root['operations'] = operations
    return root


if __name__ == '__main__':
    capture('sweep1_refit_contexts', request(), [
        'generate_sweep1_refit_contexts.py', 'generate_sweep1_refit.py',
        'generate_sweep1_rebuild.py', 'sweep1_probe.py', 'rhino_worker.py',
        'client.py', 'structural_sdk.py'],
        'VibocerosOracleSweepRefitContexts20261028', timeout=300)
