"""Probe bounded public-SDK search arithmetic without desktop windows."""
from .capture_family import capture
from .arc_blend_sdk import request,SEARCH_RULES
if __name__=='__main__':
 capture('arc_blend_search_rules',request(list(SEARCH_RULES)),['generate_arc_blend_search_rules.py','arc_blend_search_rules.py','arc_blend_roundoff.py','arc_blend_sdk.py','structural_sdk.py','rhino_worker.py','client.py','merge_edges_probe.py'],'VibocerosOracleArcBlendSearchRules20261106',timeout=1200)
