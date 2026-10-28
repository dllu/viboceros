"""Test rational storage flags without changing any unit-weight input geometry."""
from .capture_family import capture
from .generate_sweep1_refit_stage_reentry import request as reentry


def request():
    query = reentry()
    for operation in query['operations']:
        operation['id'] += '-polynomial'
        operation['polynomial_profiles'] = True
    return query


if __name__ == '__main__':
    capture('sweep1_refit_polynomial', request(), [
        'generate_sweep1_refit_polynomial.py', 'generate_sweep1_refit_stage_reentry.py',
        'capture_sources/sweep1_refit_c1_profiles.json', 'generate_sweep1_refit.py',
        'generate_sweep1_rebuild.py', 'sweep1_probe.py', 'rhino_worker.py',
        'client.py', 'structural_sdk.py'],
        'VibocerosOracleSweepRefitPolynomial20261028', timeout=300)
