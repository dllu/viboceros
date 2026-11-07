"""Capture supporting-face and mixed-source holdouts in private Xvfb."""
from .capture_family import capture
from .blend_curve_command import request,EDGE_HOLDOUTS
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_edge_holdouts',request(list(EDGE_HOLDOUTS)),[
  'generate_blend_curve_edge_holdouts.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(n+'.py'for n in(module,*deps)),
 ],'VibocerosOracleBlendCurveEdgeHoldouts20261107',timeout=2000)
