"""Capture singular chart orientation, reversal and relative coincidence."""
from .capture_family import capture
from .srf_pt_cases import request,SINGULAR
if __name__=='__main__':
    capture('srf_pt_singular',request(list(SINGULAR)),['generate_srf_pt_singular.py','srf_pt_probe.py',
        'structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleSrfPtSingular20261106',timeout=1000)
