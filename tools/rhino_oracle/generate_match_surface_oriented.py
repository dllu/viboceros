"""Capture all natural side orientations and independently repeat final cases."""
from .capture_family import capture
from .match_surface_cases import ORIENTED,request


if __name__=='__main__':
    capture('match_surface_oriented',request(list(ORIENTED)),[
        'generate_match_surface_oriented.py','match_surface_cases.py','match_surface_sdk.py',
        'structural_sdk.py','rhino_worker.py','client.py','brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleMatchSurfaceOriented20261108',timeout=1200)
