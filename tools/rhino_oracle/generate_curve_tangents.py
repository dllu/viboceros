"""Endpoint, repeated and interior tangent batches with explicit source clocks."""
import copy
import json
from pathlib import Path
from .capture_family import capture
from .generate_loft_refit_endpoint_precision import request as endpoints

HERE = Path(__file__).resolve().parent
FAMILY = 'curve_tangents'
INPUTS = 'fixtures/loft_refit_coarse.json'


def request():
    precision = endpoints()
    sources = [(op['id'],op['curves'][0]) for op in precision['operations']]
    coarse = json.loads((HERE / INPUTS).read_text())
    for shape in ['quarter','quadratic','polynomial_cubic','smooth_cubic','quintic','circle']:
        source = next(op['curves'][0] for op in coarse['operations']
                      if op['id'] == 'loft-refit-coarse-'+shape+'-100.0')
        sources.append((shape,source))
    sources.append(('polyline',dict(degree=1,knots=[0.,0.,1.,2.,2.],control_points=[
        dict(point=p,weight=1.) for p in [[0.,0.,0.],[2.,0.,0.],[2.,3.,0.]]])))
    operations = []
    for name,source in sources:
        start,end = source['knots'][source['degree']],source['knots'][len(source['control_points'])]
        operations.append(dict(op='nurbs_curve_tangents',id='tangents-'+name,
            curve=copy.deepcopy(source),parameters=[end,start,start+(end-start)*.5,end]))
    return dict(protocol_version=1,iterations=1,tolerance=precision['tolerance'],operations=operations)


if __name__ == '__main__':
    capture(FAMILY,request(),['generate_curve_tangents.py','generate_loft_refit_endpoint_precision.py',
        INPUTS,'fixtures/sweep1_refit_opposed_cosines.json','curve_tangents_probe.py',
        'geometry_validity_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleCurveTangents20261029',timeout=300)
