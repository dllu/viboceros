"""Distinguish ordinary boundary picks from the public Edges selector."""
from .capture_family import capture
from .blend_curve_command import request,EDGE_SELECTION
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_edge_selection',request(list(EDGE_SELECTION)),[
  'generate_blend_curve_edge_selection.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(n+'.py'for n in(module,*deps)),
 ],'VibocerosOracleBlendCurveEdgeSelection20261107',timeout=1000)
