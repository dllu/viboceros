"""Capture three/four-corner surface construction with paced public input."""
from .capture_family import capture
from .srf_pt_cases import request
if __name__=='__main__':
    capture('srf_pt_pilot',request(),['generate_srf_pt_pilot.py','srf_pt_cases.py','srf_pt_probe.py',
        'conic_getter_input.py','managed_mouse_callback.py','snap_environment.py','number_token.py',
        'join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleSrfPtPilot20261106',timeout=1200)
