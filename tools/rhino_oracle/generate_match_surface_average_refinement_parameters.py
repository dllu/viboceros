"""Isolate boundary and surface inverse parameters across averaging passes."""
from .capture_family import capture
from .match_surface_average_cases import REFINEMENT_PARAMETERS,request


if __name__=='__main__':
    capture('match_surface_average_refinement_parameters',request(list(REFINEMENT_PARAMETERS)),[
        'generate_match_surface_average_refinement_parameters.py','match_surface_average_cases.py','match_surface_average_sdk.py',
        'match_surface_cases.py','match_surface_sdk.py','structural_sdk.py','rhino_worker.py','client.py',
        'brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleSurfaceAverageRefinementParameters20261108',timeout=1200)
