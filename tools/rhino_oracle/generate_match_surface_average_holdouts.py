"""Vary averaging clocks, natural edges, weights, preservation and iso modes."""
from .capture_family import capture
from .match_surface_average_cases import HOLDOUTS,request


if __name__=='__main__':
    capture('match_surface_average_holdouts',request(list(HOLDOUTS)),[
        'generate_match_surface_average_holdouts.py','match_surface_average_cases.py','match_surface_average_sdk.py',
        'match_surface_cases.py','match_surface_sdk.py','structural_sdk.py','rhino_worker.py','client.py',
        'brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleMatchSurfaceAverageHoldouts20261108',timeout=1200)
