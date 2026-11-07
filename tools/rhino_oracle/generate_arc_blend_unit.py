"""Capture public arc-blend geometry in a private Rhino session."""
from .capture_family import capture
from .arc_blend_sdk import request,HOLDOUTS
if __name__=='__main__':capture('arc_blend_unit',request(list(HOLDOUTS)),['generate_arc_blend_unit.py','arc_blend_sdk.py','structural_sdk.py','rhino_worker.py','client.py','merge_edges_probe.py'],'VibocerosOracleArcBlendUnit20261106',timeout=1200)
