"""Isolate position refinement across degrees, charts, weights and bases."""
from .capture_family import capture
from .match_surface_average_cases import POSITION_REFINEMENT,request


if __name__=='__main__':
    capture('match_surface_average_position_refinement',request(list(POSITION_REFINEMENT)),[
        'generate_match_surface_average_position_refinement.py','match_surface_average_cases.py','match_surface_average_sdk.py',
        'match_surface_cases.py','match_surface_sdk.py','structural_sdk.py','rhino_worker.py','client.py',
        'brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleSurfaceAveragePositionRefinement20261108',timeout=1200)
