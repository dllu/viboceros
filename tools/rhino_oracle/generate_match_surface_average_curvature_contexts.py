"""Isolate whole-input translation and boundary-gap effects in averaged G2."""
from .capture_family import capture
from .match_surface_average_cases import CURVATURE_CONTEXTS,request


if __name__=='__main__':
    capture('match_surface_average_curvature_contexts',request(list(CURVATURE_CONTEXTS)),[
        'generate_match_surface_average_curvature_contexts.py','match_surface_average_cases.py','match_surface_average_sdk.py',
        'match_surface_cases.py','match_surface_sdk.py','structural_sdk.py','rhino_worker.py','client.py',
        'brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleSurfaceAverageCurvatureContexts20261108',timeout=1200)
