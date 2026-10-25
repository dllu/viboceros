"""Compare translated equal-profile Loft Refit with public cubic Bezier fits."""
import copy
from .generate_loft_refit import request as base_request, HERE
from .capture_family import capture


def request():
    operations = []
    for source in base_request()['operations']:
        if source['style'] != 'straight' or source['refit_tolerance'] == 0.:
            continue
        for i, profile in enumerate(source['curves']):
            op = copy.deepcopy(source)
            other = copy.deepcopy(profile)
            for cp in other['control_points']:
                cp['point'][2] += 2.
            op.update(id=source['id'] + '-profile-' + str(i), curves=[profile, other],
                      inspect_refit_beziers=True)
            operations.append(op)
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_refit_diagnostics', request(), [
        'generate_loft_refit_diagnostics.py', 'generate_loft_refit.py',
        'generate_loft_rebuild.py', 'fixtures/loft.json', 'loft_refit_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitDiagnostics20261025')
