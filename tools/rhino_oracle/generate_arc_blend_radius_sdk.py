"""Capture public arc-blend geometry in a private Rhino session."""
from .capture_family import capture
from .arc_blend_sdk import request,RADIUS_SEARCH
if __name__=='__main__':capture('arc_blend_radius_sdk',request(list(RADIUS_SEARCH)),['generate_arc_blend_radius_sdk.py','arc_blend_sdk.py','structural_sdk.py','rhino_worker.py','client.py','merge_edges_probe.py'],'VibocerosOracleArcBlendRadiusSdk20261106',timeout=1200)
