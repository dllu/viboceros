"""Capture SrfPt arithmetic at large origins and very narrow edges."""
from .capture_family import capture
from .srf_pt_cases import request,PRECISION
if __name__=='__main__':
    capture('srf_pt_precision',request(list(PRECISION)),['generate_srf_pt_precision.py','srf_pt_probe.py',
        'structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleSrfPtPrecision20261106',timeout=1000)
