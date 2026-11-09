"""Capture complete two-sided matches without reading proprietary code."""
from .capture_family import capture
from .match_surface_average_cases import PILOT,request


if __name__=='__main__':
    capture('match_surface_average_pilot',request(list(PILOT)),[
        'generate_match_surface_average_pilot.py','match_surface_average_cases.py','match_surface_average_sdk.py',
        'match_surface_cases.py','match_surface_sdk.py','structural_sdk.py','rhino_worker.py','client.py',
        'brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleMatchSurfaceAveragePilot20261108',timeout=1200)
