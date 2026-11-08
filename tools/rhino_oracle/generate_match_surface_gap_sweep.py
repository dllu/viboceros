"""Capture custom surface matching accuracy and closest-contact errors."""
from .capture_family import capture
from .match_surface_cases import GAP_SWEEP,request


if __name__=='__main__':
    capture('match_surface_gap_sweep',request(list(GAP_SWEEP)),[
        'generate_match_surface_gap_sweep.py','match_surface_cases.py','match_surface_sdk.py',
        'structural_sdk.py','rhino_worker.py','client.py','brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleMatchSurfaceGapSweep20261108',timeout=1200)
