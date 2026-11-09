"""Isolate rational nonuniform cross-knot behavior in averaged G2."""
from .capture_family import capture
from .match_surface_average_cases import CURVATURE_NONUNIFORM,request


if __name__=='__main__':
    capture('match_surface_average_curvature_nonuniform',request(list(CURVATURE_NONUNIFORM)),[
        'generate_match_surface_average_curvature_nonuniform.py','match_surface_average_cases.py','match_surface_average_sdk.py',
        'match_surface_cases.py','match_surface_sdk.py','structural_sdk.py','rhino_worker.py','client.py',
        'brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleSurfaceAverageCurvatureNonuniform20261108',timeout=1200)
