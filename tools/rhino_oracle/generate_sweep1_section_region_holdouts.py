"""Retained rails and point-ended sweeps with unequal authored regions."""
import copy
from .capture_family import capture
from .generate_sweep1_section_regions import request as original


def request():
    query = original()
    operations = []
    for op in query['operations']:
        if op['command']:
            retained = copy.deepcopy(op)
            retained.update(id=op['id'] + '-retained', refit_rail=False)
            operations.append(retained)
    for command in [False, True]:
        base = next(op for op in query['operations']
                    if op['id'] == 'unequal-corners-' + ('command' if command else 'sdk'))
        for both in [False, True]:
            op = copy.deepcopy(base)
            op['id'] += '-both-poles' if both else '-end-pole'
            op['refit_rail'] = False
            op['parameters'] = [1., 3.] if both else [0., 3.]
            for section, z in zip(op['sections'], op['parameters']):
                for point in section['vertices']:
                    point[2] = z
            op['end_point'] = [0., 0., 5.]
            if both:
                op['start_point'] = [0., 0., 0.]
            operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('sweep1_section_region_holdouts', request(), [
        'generate_sweep1_section_region_holdouts.py', 'generate_sweep1_section_regions.py',
        'sweep1_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleSweepRegionHoldouts20261028', timeout=300)
