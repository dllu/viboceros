"""Isolate curvature-sign versus magnitude stopping decisions."""
from .capture_family import capture
from .match_surface_average_cases import REFINEMENT_SIGNS,request


if __name__=='__main__':
    capture('match_surface_average_refinement_signs',request(list(REFINEMENT_SIGNS)),[
        'generate_match_surface_average_refinement_signs.py','match_surface_average_cases.py','match_surface_average_sdk.py',
        'match_surface_cases.py','match_surface_sdk.py','structural_sdk.py','rhino_worker.py','client.py',
        'brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleSurfaceAverageRefinementSigns20261108',timeout=1200)
