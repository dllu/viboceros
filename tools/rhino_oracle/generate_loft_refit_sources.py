"""Check simplification and input count before the independent public fitters."""
import copy
from .capture_family import capture
from .generate_loft_refit_policy import request as policy_request


def request():
    query = policy_request()
    query['operations'] = [copy.deepcopy(o) for o in query['operations']
        if o['refit_tolerance'] > 0. and '-angle-' not in o['id'] and '-domain-' not in o['id']
        and any('-' + name + '-' in o['id'] for name in ['quarter', 'cubic', 'polynomial_cubic'])]
    for op in query['operations']:
        op['id'] = op['id'].replace('loft-refit-policy-', 'loft-refit-sources-')
        op['inspect_refit_sources'] = True
    return query


if __name__ == '__main__':
    capture('loft_refit_sources', request(), ['generate_loft_refit_sources.py',
        'generate_loft_refit_policy.py', 'generate_loft_refit.py', 'generate_loft_rebuild.py',
        'fixtures/loft.json', 'loft_refit_source_probe.py', 'loft_refit_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitSources20261025')
