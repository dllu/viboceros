"""Isolate the section fitter and public length-parameter clocks for Loft Refit."""
import copy
from .capture_family import capture
from .generate_loft_refit_policy import request as policy_request


def request():
    query = policy_request()
    query['operations'] = [copy.deepcopy(o) for o in query['operations']
        if o['refit_tolerance'] > 0. and '-angle-' not in o['id'] and '-domain-' not in o['id']
        and not any('-' + name + '-' in o['id'] for name in ['line', 'uneven_line'])]
    for op in query['operations']:
        op['id'] = op['id'].replace('loft-refit-policy-', 'loft-refit-clock-')
        op['inspect_refit_clock'] = True
    return query


if __name__ == '__main__':
    capture('loft_refit_clock', request(), ['generate_loft_refit_clock.py',
        'generate_loft_refit_policy.py', 'generate_loft_refit.py', 'generate_loft_rebuild.py',
        'fixtures/loft.json', 'loft_refit_clock_probe.py', 'rhino_worker.py', 'client.py',
        'structural_sdk.py'], 'VibocerosOracleLoftRefitClock20261025')
