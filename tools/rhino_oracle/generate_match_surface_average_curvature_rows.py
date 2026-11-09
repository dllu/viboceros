"""Isolate independent curvature-row changes in averaged G2."""
from .capture_family import capture
from .match_surface_average_cases import CURVATURE_ROWS,request


if __name__=='__main__':
    capture('match_surface_average_curvature_rows',request(list(CURVATURE_ROWS)),[
        'generate_match_surface_average_curvature_rows.py','match_surface_average_cases.py','match_surface_average_sdk.py',
        'match_surface_cases.py','match_surface_sdk.py','structural_sdk.py','rhino_worker.py','client.py',
        'brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleSurfaceAverageCurvatureRows20261108',timeout=1200)
