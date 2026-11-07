"""Capture public arc-blend geometry in a private Rhino session."""
from .capture_family import capture
from .arc_blend_sdk import request
if __name__=='__main__':capture('arc_blend_sdk',request(),['generate_arc_blend_sdk.py','arc_blend_sdk.py','structural_sdk.py','rhino_worker.py','client.py','merge_edges_probe.py'],'VibocerosOracleArcBlendSdk20261106',timeout=1200)
