"""Freeze independent profiles, angular counts and public profile diagnostics."""
from .capture_family import capture
from .revolve_subd_cases import holdouts
from .revolve_subd_profile_probe import request
if __name__=='__main__':
    query=request(True);query['operations']=holdouts()+query['operations']
    capture('revolve_subd_holdouts',query,
        ['generate_revolve_subd_holdouts.py','revolve_subd_profile_probe.py','revolve_subd_probe.py','revolve_subd_cases.py','revolve_splitting_cases.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRevolveSubDHoldouts20261106',timeout=1200)
