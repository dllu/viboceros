"""Capture spatial and degenerate SrfPt input boundaries."""
from .capture_family import capture
from .srf_pt_cases import request,HOLDOUTS
if __name__=='__main__':
    capture('srf_pt_holdouts',request(list(HOLDOUTS)),['generate_srf_pt_holdouts.py','srf_pt_probe.py',
        'structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleSrfPtHoldouts20261106',timeout=1200)
