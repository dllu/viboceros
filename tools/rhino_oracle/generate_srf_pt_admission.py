"""Capture the native corner and line-edge admission boundaries."""
from .capture_family import capture
from .srf_pt_cases import request,ADMISSION
if __name__=='__main__':
    capture('srf_pt_admission',request(list(ADMISSION)),['generate_srf_pt_admission.py','srf_pt_probe.py',
        'structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleSrfPtAdmission20261106',timeout=1000)
