"""Test a public curve-matching operation as a positional refinement hypothesis."""
from .capture_family import capture
from .match_surface_cases import PREFIT_AUDIT,request


if __name__=='__main__':
    capture('match_surface_prefit_audit',request(list(PREFIT_AUDIT)),[
        'generate_match_surface_prefit_audit.py','match_surface_cases.py','match_surface_sdk.py',
        'structural_sdk.py','rhino_worker.py','client.py','brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleMatchSurfacePrefitAudit20261108',timeout=1200)
