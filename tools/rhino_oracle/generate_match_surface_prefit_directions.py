"""Check curve direction and distance limits in public MatchToCurve calls."""
from .capture_family import capture
from .match_surface_cases import PREFIT_DIRECTIONS,request


if __name__=='__main__':
    capture('match_surface_prefit_directions',request(list(PREFIT_DIRECTIONS)),[
        'generate_match_surface_prefit_directions.py','match_surface_cases.py','match_surface_sdk.py',
        'structural_sdk.py','rhino_worker.py','client.py','brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleMatchSurfacePrefitDirections20261108',timeout=1200)
