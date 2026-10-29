"""Compare original inputs and independently constructed proposal re-entry.

These public-frame cubics are controls, not claimed native intermediates.
"""
import copy
import json
import math
from pathlib import Path
from .capture_family import capture

HERE=Path(__file__).resolve().parent
FAMILY='sweep1_refit_proposal_reentry'
FIXTURE='fixtures/loft_refit_endpoint_precision.json'
OBSERVATION='observations/loft_refit_endpoint_precision.json'


def request():
    query=json.loads((HERE/FIXTURE).read_text())
    native=json.loads((HERE/OBSERVATION).read_text())
    operations=[]
    for op,row in zip(query['operations'],native['results']):
        assert op['id']==row['id']
        label=op['id'].removeprefix('endpoint-precision-')
        diagnostic=row['value']['refit_adjustment']
        zero=copy.deepcopy(diagnostic['initial'])
        signed=copy.deepcopy(zero)
        first,last=[zero['control_points'][i]['point'] for i in [0,3]]
        length=math.dist(first,last)
        axis=diagnostic['chord_direction']
        for i,tangent in enumerate(diagnostic['endpoint_tangents']):
            cosine=max(-1.,min(1.,sum(a*b for a,b in zip(axis,tangent))))
            handle=length*(2.-cosine*cosine)/3.
            signed['control_points'][1 if i==0 else 2]['point']=[
                p+(-1. if i else 1.)*handle*v for p,v in zip(first if i==0 else last,tangent)]
        for variant,source in [('original',op['curves'][0]),('zero',zero),('signed',signed)]:
            other=copy.deepcopy(source)
            for control in other['control_points']:control['point'][2]+=5.
            for count in [1,2]:
                operations.append(dict(op='sweep_refit_sections',
                    id='reentry-{}-{}-{}'.format(label,variant,count),
                    sections=copy.deepcopy([source,other][:count]),parameters=[-7.,13.][:count],
                    refit_tolerance=100.))
    return dict(protocol_version=1,iterations=1,tolerance=query['tolerance'],operations=operations)


if __name__=='__main__':
    capture(FAMILY,request(),['generate_sweep1_refit_proposal_reentry.py',FIXTURE,OBSERVATION,
        'sweep_refit_sections_probe.py','geometry_validity_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleProposalReentry20261029',timeout=300)
