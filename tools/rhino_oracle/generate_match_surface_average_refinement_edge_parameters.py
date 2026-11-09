"""Isolate edge-origin and surface-origin inverse parameters."""
from .capture_family import capture
from .match_surface_average_cases import REFINEMENT_EDGE_PARAMETERS,request


if __name__=='__main__':
    capture('match_surface_average_refinement_edge_parameters',request(list(REFINEMENT_EDGE_PARAMETERS)),[
        'generate_match_surface_average_refinement_edge_parameters.py','match_surface_average_cases.py','match_surface_average_sdk.py',
        'match_surface_cases.py','match_surface_sdk.py','structural_sdk.py','rhino_worker.py','client.py',
        'brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleSurfaceAverageRefinementEdgeParameters20261108',timeout=1200)
