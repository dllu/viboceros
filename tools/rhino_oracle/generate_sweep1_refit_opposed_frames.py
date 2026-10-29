"""Distinguish exact and nearly opposed endpoint-tangent admission."""
import copy
import json
import math
from pathlib import Path
from .capture_family import capture

HERE = Path(__file__).resolve().parent
FAMILY = 'sweep1_refit_opposed_frames'
INPUTS = 'fixtures/sweep1_refit_endpoint_frames.json'


def request():
    base = json.loads((HERE / INPUTS).read_text())
    original = next(op for op in base['operations']
        if op['id'] == 'endpoint-frame-start--0.5-count-1')
    operations = []
    for delta in [0.,.001,.01,.02,.05,1e-6,1e-8,1e-10,1e-12]:
        source = copy.deepcopy(original['sections'][0])
        cosine = .5 + delta
        source['control_points'][2]['point'] = [4.-cosine,math.sqrt(1.-cosine*cosine),0.]
        second = copy.deepcopy(source)
        for control in second['control_points']:
            control['point'][2] += 5.
        for count in [1,2]:
            operations.append(dict(op='sweep_refit_sections',
                id='opposed-frame-{}-count-{}'.format(delta,count),
                sections=copy.deepcopy([source,second][:count]),parameters=[-7.,13.][:count],
                refit_tolerance=100.))
    return dict(protocol_version=1,iterations=1,tolerance=base['tolerance'],operations=operations)


if __name__ == '__main__':
    capture(FAMILY,request(),['generate_sweep1_refit_opposed_frames.py',INPUTS,
        'sweep_refit_sections_probe.py','geometry_validity_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleOpposedFrames20261029',timeout=300)
