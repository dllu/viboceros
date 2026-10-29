"""Public endpoint frames around the fitting opposition cutoff."""
import copy
import json
from pathlib import Path
from .capture_family import capture

HERE = Path(__file__).resolve().parent
FAMILY = 'loft_refit_endpoint_precision'
INPUTS = 'fixtures/sweep1_refit_opposed_cosines.json'


def request():
    base = json.loads((HERE / INPUTS).read_text())
    originals = [op for op in base['operations'] if len(op['sections']) == 1
                 and any(k in op['id'] for k in ['--0.9999001-', '--0.9999-', '--0.9998999-'])]
    sources = [(op['id'].replace('-count-1',''), copy.deepcopy(op['sections'][0])) for op in originals]
    center = next(source for name,source in sources if name == 'opposed-cosine--0.9999')
    for name in ['translated', 'rotated', 'scaled', 'gauged']:
        source = copy.deepcopy(center)
        for control in source['control_points']:
            x,y,z = control['point']
            if name == 'translated': control['point'] = [7.+x,-3.+y,2.+z]
            elif name == 'rotated': control['point'] = [y,z,x]
            elif name == 'scaled': control['point'] = [2.*x,2.*y,2.*z]
            else: control['weight'] *= -7.
        if name == 'gauged': source['knots'] = [-7.+20.*t for t in source['knots']]
        sources.append((name,source))
    operations = []
    for name,source in sources:
        other = copy.deepcopy(source)
        for control in other['control_points']: control['point'][2] += 5.
        operations.append(dict(op='loft',id='endpoint-precision-'+name,
            curves=[source,other],style='straight',closed=False,
            refit_tolerance=100.,inspect_inputs=True,inspect_refit_adjustment=True,
            sample_geometry=False,allow_empty=True))
    return dict(protocol_version=1,iterations=1,tolerance=base['tolerance'],operations=operations)


if __name__ == '__main__':
    capture(FAMILY,request(),['generate_loft_refit_endpoint_precision.py',INPUTS,
        'loft_refit_adjustment_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleEndpointPrecision20261029',timeout=300)
