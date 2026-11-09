"""Separate within-fit approximation from subsequent paired averaging."""
from .capture_family import capture
from .match_surface_average_cases import FIT_STAGES,request


if __name__=='__main__':
    capture('match_surface_average_fit_stages',request(list(FIT_STAGES)),[
        'generate_match_surface_average_fit_stages.py','match_surface_average_cases.py','match_surface_average_sdk.py',
        'match_surface_cases.py','match_surface_sdk.py','structural_sdk.py','rhino_worker.py','client.py',
        'brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleSurfaceAverageFitStages20261108',timeout=1200)
