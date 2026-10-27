"""Complete public surface records for independent Sweep1 placement analysis."""
from .capture_family import capture
from .generate_sweep1_section_placement import request as placement_request


def request():
    query = placement_request()
    for op in query['operations']:
        op['id'] += '-definition'
        op['inspect_definition'] = True
        op['queries'] = [[x, 0., z] for z in [-2., 0., 1., 2., 3., 4., 5., 6., 7., 8., 10., 12.]
                         for x in [0., 1., 2., 3., 4.]]
    return query


if __name__ == '__main__':
    capture('sweep1_section_definitions', request(), [
        'generate_sweep1_section_definitions.py', 'generate_sweep1_section_placement.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleSweep1Definitions20261027')
