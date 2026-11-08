"""Audit actual refinement tolerances and closest-contact errors."""
from .capture_family import capture
from .match_surface_cases import REFINEMENT,request


if __name__=='__main__':
    capture('match_surface_refinement',request(list(REFINEMENT)),[
        'generate_match_surface_refinement.py','match_surface_cases.py','match_surface_sdk.py',
        'structural_sdk.py','rhino_worker.py','client.py','brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleMatchSurfaceRefinement20261108',timeout=1200)
