"""Isolate iso, preservation, degree, and chart holdouts in averaged G2."""
from .capture_family import capture
from .match_surface_average_cases import CURVATURE_HOLDOUTS,request


if __name__=='__main__':
    capture('match_surface_average_curvature_holdouts',request(list(CURVATURE_HOLDOUTS)),[
        'generate_match_surface_average_curvature_holdouts.py','match_surface_average_cases.py','match_surface_average_sdk.py',
        'match_surface_cases.py','match_surface_sdk.py','structural_sdk.py','rhino_worker.py','client.py',
        'brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleSurfaceAverageCurvatureHoldouts20261108',timeout=1200)
