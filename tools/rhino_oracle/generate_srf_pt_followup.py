"""Record public pre-admission orientation and remote-coordinate admission."""
from .capture_family import capture
from .srf_pt_cases import request,FOLLOWUP
if __name__=='__main__':
    capture('srf_pt_followup',request(list(FOLLOWUP)),['generate_srf_pt_followup.py','srf_pt_probe.py',
        'structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleSrfPtFollowup20261106',timeout=1200)
