"""Endpoint admission around one degree of departure from opposite tangents."""
import copy
import json
import math
from pathlib import Path
from .capture_family import capture

HERE = Path(__file__).resolve().parent
FAMILY = 'sweep1_refit_opposed_boundary'
INPUTS = 'fixtures/sweep1_refit_endpoint_frames.json'


def request():
    base = json.loads((HERE / INPUTS).read_text())
    original = next(op for op in base['operations']
        if op['id'] == 'endpoint-frame-start--0.5-count-1')
    operations = []
    for degrees in [.99,1.,1.01]:
        cosine = math.cos(-math.pi/3. + math.radians(degrees))
        source = copy.deepcopy(original['sections'][0])
        source['control_points'][2]['point'] = [4.-cosine,math.sqrt(1.-cosine*cosine),0.]
        second = copy.deepcopy(source)
        for control in second['control_points']:
            control['point'][2] += 5.
        for count in [1,2]:
            operations.append(dict(op='sweep_refit_sections',
                id='opposed-boundary-{}-count-{}'.format(degrees,count),
                sections=copy.deepcopy([source,second][:count]),parameters=[-7.,13.][:count],
                refit_tolerance=100.))
    return dict(protocol_version=1,iterations=1,tolerance=base['tolerance'],operations=operations)


if __name__ == '__main__':
    capture(FAMILY,request(),['generate_sweep1_refit_opposed_boundary.py',INPUTS,
        'sweep_refit_sections_probe.py','geometry_validity_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleOpposedBoundary20261029',timeout=300)
