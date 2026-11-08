"""Capture custom surface matching accuracy and closest-contact errors."""
from .capture_family import capture
from .match_surface_cases import DEVIATION_QUERY_AUDIT,request


if __name__=='__main__':
    capture('match_surface_deviation_query_audit',request(list(DEVIATION_QUERY_AUDIT)),[
        'generate_match_surface_deviation_query_audit.py','match_surface_cases.py','match_surface_sdk.py',
        'structural_sdk.py','rhino_worker.py','client.py','brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleMatchSurfaceDeviationQueryAudit20261108',timeout=1200)
