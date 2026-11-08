"""Measure rational, degree, averaging, closest-point and refinement variants."""
from .capture_family import capture
from .match_surface_cases import HOLDOUTS,request


if __name__=='__main__':
    capture('match_surface_holdouts',request(list(HOLDOUTS)),[
        'generate_match_surface_holdouts.py','match_surface_cases.py','match_surface_sdk.py',
        'structural_sdk.py','rhino_worker.py','client.py','brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleMatchSurfaceHoldouts20261108',timeout=1200)
