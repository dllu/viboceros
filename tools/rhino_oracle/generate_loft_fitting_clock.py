"""Direct scalar-clock transport at the observed smooth-fitting precision."""
import copy
from .capture_family import capture
from .generate_loft_scalar_clock import request as sources


def request():
    query = sources()
    query['operations'] = [dict(op='curve_length_parameters',
        id=op['id'].replace('loft-scalar-clock-', 'loft-fitting-clock-'),
        curve=copy.deepcopy(op['curves'][0]),
        fractions=[1., .5, 0., .125, 63. / 64., .375, .5, .75, 1. / 64., .875, .625, .25])
        for op in query['operations']]
    return query


if __name__ == '__main__':
    capture('loft_fitting_clock', request(), ['generate_loft_fitting_clock.py',
        'generate_loft_scalar_clock.py', 'generate_loft_refit_clock.py', 'generate_loft_refit_policy.py',
        'generate_loft_refit.py', 'generate_loft_rebuild.py', 'generate_loft_refit_line_inputs.py',
        'generate_loft_refit_polylines.py', 'fixtures/loft.json', 'loft_scalar_clock_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'], 'VibocerosOracleLoftFittingClock20261025')
