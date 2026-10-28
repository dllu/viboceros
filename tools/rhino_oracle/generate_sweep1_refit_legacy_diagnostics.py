"""Compare public SweepOneRail and Brep.CreateFromSweep Refit outputs."""
import copy

from .capture_family import capture
from .generate_sweep1_refit import request as sdk
from .generate_sweep1_refit_common_handles import request as handles
from .generate_sweep1_refit_holdouts import request as holdouts


def request():
    root = sdk()
    sources = {op['id']: op for r in [root, handles(), holdouts()]
               for op in r['operations']}
    root['operations'] = []
    for name in ['open-refit-1e-06', 'closed-refit-1e-06',
                 'open-refit-0.001', 'closed-cubic-0.001',
                 'common-0.99-1-0.001', 'common-0.99-1-0.01']:
        for legacy in [False, True]:
            op = copy.deepcopy(sources[name])
            op.update(id=name + ('-legacy' if legacy else '-modern'),
                      legacy_refit=legacy)
            root['operations'].append(op)
    return root


if __name__ == '__main__':
    capture('sweep1_refit_legacy_diagnostics', request(), [
        'generate_sweep1_refit_legacy_diagnostics.py',
        'generate_sweep1_refit.py', 'generate_sweep1_rebuild.py',
        'generate_sweep1_refit_common_handles.py',
        'generate_sweep1_refit_holdouts.py', 'sweep1_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleSweepRefitLegacy20261028', timeout=300)
