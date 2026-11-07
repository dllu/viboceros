"""Isolate a single whole-circle contact in private Xvfb."""
from .capture_family import capture
from .blend_curve_command import request,CLOSED_SINGLE
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_closed_single',request(list(CLOSED_SINGLE)),[
  'generate_blend_curve_closed_single.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(n+'.py'for n in(module,*deps)),
 ],'VibocerosOracleBlendClosedSingle20261107',timeout=500)
