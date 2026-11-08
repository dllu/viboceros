"""Capture custom surface matching accuracy and closest-contact errors."""
from .capture_family import capture
from .match_surface_cases import TOLERANCE_HOLDOUTS,request


if __name__=='__main__':
    capture('match_surface_tolerances_sdk',request(list(TOLERANCE_HOLDOUTS)),[
        'generate_match_surface_tolerances_sdk.py','match_surface_cases.py','match_surface_sdk.py',
        'structural_sdk.py','rhino_worker.py','client.py','brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleMatchSurfaceToleranceSdk20261108',timeout=1200)
