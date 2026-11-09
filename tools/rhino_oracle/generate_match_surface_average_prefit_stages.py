"""Compare public surface-to-curve fitting with averaged refinement bases."""
from .capture_family import capture
from .match_surface_average_cases import PREFIT_STAGES,request


if __name__=='__main__':
    capture('match_surface_average_prefit_stages',request(list(PREFIT_STAGES)),[
        'generate_match_surface_average_prefit_stages.py','match_surface_average_cases.py','match_surface_average_sdk.py',
        'match_surface_cases.py','match_surface_sdk.py','structural_sdk.py','rhino_worker.py','client.py',
        'brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleSurfaceAveragePrefitStages20261108',timeout=1200)
