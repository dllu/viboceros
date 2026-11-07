"""Compare independent floating-point formulas in a private public-SDK scene."""
from .capture_family import capture
from .arc_blend_sdk import request,ROUNDOFF
if __name__=='__main__':
 capture('arc_blend_roundoff',request(list(ROUNDOFF)),['generate_arc_blend_roundoff.py','arc_blend_roundoff.py','arc_blend_sdk.py','structural_sdk.py','rhino_worker.py','client.py','merge_edges_probe.py'],'VibocerosOracleArcBlendRoundoff20261106',timeout=1200)
