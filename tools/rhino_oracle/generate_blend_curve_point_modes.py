"""Capture public BlendCrv Point endpoint derivatives, movement, joining and cancellation."""
from .capture_family import capture
from .blend_curve_command import request,POINT_MODES
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_point_modes',request(list(POINT_MODES)),[
  'generate_blend_curve_point_modes.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(n+'.py'for n in(module,*deps)),
 ],'VibocerosOracleBlendCurvePointModes20261107',timeout=1600)
