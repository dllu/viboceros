"""Capture origin input in world and rotated construction planes."""
from .capture_family import capture
from .srf_pt_cases import request,ORIGINS
if __name__=='__main__':
    capture('srf_pt_origin',request(list(ORIGINS)),['generate_srf_pt_origin.py','srf_pt_probe.py',
        'structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleSrfPtOrigin20261106',timeout=1000)
