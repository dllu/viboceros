"""Isolate public-API averaging composition for refinement diagnostics."""
from .capture_family import capture
from .match_surface_average_cases import REFINEMENT_STEPS,request


if __name__=='__main__':
    capture('match_surface_average_refinement_steps',request(list(REFINEMENT_STEPS)),[
        'generate_match_surface_average_refinement_steps.py','match_surface_average_cases.py','match_surface_average_sdk.py',
        'match_surface_cases.py','match_surface_sdk.py','structural_sdk.py','rhino_worker.py','client.py',
        'brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleSurfaceAverageRefinementSteps20261108',timeout=1200)
