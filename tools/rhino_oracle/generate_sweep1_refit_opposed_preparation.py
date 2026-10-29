"""Retain public no-fit preparation charts as independent fitting controls."""
import copy
import json
from pathlib import Path
from .capture_family import capture

HERE=Path(__file__).resolve().parent
FAMILY='sweep1_refit_opposed_preparation'
INPUTS='fixtures/loft_refit_endpoint_precision.json'


def request():
    source=json.loads((HERE/INPUTS).read_text());operations=[]
    for op in source['operations']:
        label=op['id'].removeprefix('endpoint-precision-')
        for count in [1,2]:
            operations.append(dict(op='sweep_refit_sections',id='opposed-prepare-{}-{}'.format(label,count),
                sections=copy.deepcopy(op['curves'][:count]),parameters=[-7.,13.][:count],refit_tolerance=0.))
    return dict(protocol_version=1,iterations=1,tolerance=source['tolerance'],operations=operations)


if __name__=='__main__':
    capture(FAMILY,request(),['generate_sweep1_refit_opposed_preparation.py',INPUTS,
        'sweep_refit_sections_probe.py','geometry_validity_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleOpposedPreparation20261029',timeout=300)
