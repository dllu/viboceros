"""Distinguish averaged refinement from independent one-sided fitting."""
from .capture_family import capture
from .match_surface_average_cases import FIT_ONE_SIDED,request


if __name__=='__main__':
    capture('match_surface_average_fit_one_sided',request(list(FIT_ONE_SIDED)),[
        'generate_match_surface_average_fit_one_sided.py','match_surface_average_cases.py','match_surface_average_sdk.py',
        'match_surface_cases.py','match_surface_sdk.py','structural_sdk.py','rhino_worker.py','client.py',
        'brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleSurfaceAverageFitOneSided20261108',timeout=1200)
