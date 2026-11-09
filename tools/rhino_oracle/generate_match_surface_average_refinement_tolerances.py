"""Isolate averaged refinement pass boundaries with tolerance sweeps."""
from .capture_family import capture
from .match_surface_average_cases import REFINEMENT_TOLERANCES,request


if __name__=='__main__':
    capture('match_surface_average_refinement_tolerances',request(list(REFINEMENT_TOLERANCES)),[
        'generate_match_surface_average_refinement_tolerances.py','match_surface_average_cases.py','match_surface_average_sdk.py',
        'match_surface_cases.py','match_surface_sdk.py','structural_sdk.py','rhino_worker.py','client.py',
        'brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleSurfaceAverageRefinementTolerances20261108',timeout=1200)
