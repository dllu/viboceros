"""Capture custom surface matching accuracy and closest-contact errors."""
from .capture_family import capture
from .match_surface_cases import LOCAL_QUERY_AUDIT,request


if __name__=='__main__':
    capture('match_surface_local_query_audit',request(list(LOCAL_QUERY_AUDIT)),[
        'generate_match_surface_local_query_audit.py','match_surface_cases.py','match_surface_sdk.py',
        'structural_sdk.py','rhino_worker.py','client.py','brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleMatchSurfaceLocalQueryAudit20261108',timeout=1200)
