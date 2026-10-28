"""Compare public Curve.Fit with the single-section Sweep1 refitting path."""
import copy
from .capture_family import capture
from .generate_sweep1_refit import request as sweep_request
from .generate_sweep1_refit_holdouts import request as holdout_request


def request():
    root = sweep_request()
    curves = {name: next(op for op in root['operations']
                         if op['id'] == name + '-refit-0.01')['sections'][0]
              for name in ['open', 'rational']}
    holdouts = holdout_request()
    curves['circle'] = next(op for op in holdouts['operations']
                            if op['id'] == 'nurbs-circle-0.001')['sections'][0]
    curves['closed-cubic'] = next(op for op in holdouts['operations']
                                  if op['id'] == 'closed-cubic-0.001')['sections'][0]
    operations = []
    for name, curve in curves.items():
        for tolerance in [1e-6, .001, .01, .1]:
            operations.append(dict(op='curve_fit_geometry',
                                   id=name + '-fit-' + str(tolerance),
                                   curve=copy.deepcopy(curve), degree=3,
                                   fit_tolerance=tolerance,
                                   angle_tolerance_radians=0.))
    root['operations'] = operations
    return root


if __name__ == '__main__':
    capture('sweep1_refit_fit_diagnostics', request(), [
        'generate_sweep1_refit_fit_diagnostics.py', 'generate_sweep1_refit.py',
        'generate_sweep1_refit_holdouts.py', 'generate_sweep1_rebuild.py',
        'generate_sweep1_rebuild_holdouts.py', 'rhino_worker.py', 'client.py',
        'structural_sdk.py'], 'VibocerosOracleSweepRefitFit20261028', timeout=300)
