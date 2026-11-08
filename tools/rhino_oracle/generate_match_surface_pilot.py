"""Measure surface edge matching continuity and opposite-end constraints."""
from .capture_family import capture
from .match_surface_cases import PILOT,request


if __name__=='__main__':
    capture('match_surface_pilot',request(list(PILOT)),[
        'generate_match_surface_pilot.py','match_surface_cases.py','match_surface_sdk.py',
        'structural_sdk.py','rhino_worker.py','client.py','brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleMatchSurfacePilot20261108',timeout=1200)
