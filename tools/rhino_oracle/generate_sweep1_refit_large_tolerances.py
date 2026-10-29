"""Measure fitting at and around the endpoint-chord tolerance boundary."""
import copy
import json
import math
from pathlib import Path
from .capture_family import capture

HERE = Path(__file__).resolve().parent
FAMILY = 'sweep1_refit_large_tolerances'
INPUTS = 'fixtures/loft_refit_coarse.json'


def request():
    base = json.loads((HERE / INPUTS).read_text())
    originals = {op['id'][len('loft-refit-coarse-'):-len('-100.0')]: op['curves'][0]
        for op in base['operations'] if op['refit_tolerance'] == 100.}
    operations = []
    names = ['quarter','cubic','quadratic','polynomial_cubic','smooth_cubic','quintic','circle']
    for name in names:
        source = originals[name]
        chord = math.dist(source['control_points'][0]['point'], source['control_points'][-1]['point'])
        tolerances = [('wide',100.)] if chord == 0. else [
            ('below',.99*chord),('equal',chord),('above',1.01*chord),('wide',100.)]
        for label, tolerance in tolerances:
            second = copy.deepcopy(source)
            for control in second['control_points']:
                control['point'][2] += 5.
            for count in [1, 2]:
                operations.append(dict(op='sweep_refit_sections',
                    id='large-refit-{}-{}-count-{}'.format(name,label,count),
                    sections=copy.deepcopy([source,second][:count]),
                    parameters=[-7.,13.][:count], refit_tolerance=tolerance))
    return dict(protocol_version=1,iterations=1,tolerance=base['tolerance'],operations=operations)


if __name__ == '__main__':
    capture(FAMILY, request(), ['generate_sweep1_refit_large_tolerances.py', INPUTS,
        'sweep_refit_sections_probe.py','geometry_validity_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleLargeRefit20261029',timeout=300)
