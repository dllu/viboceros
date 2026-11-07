"""Repeat holdouts with disjoint picks and explicit initial display state."""
from .capture_family import capture
from .blend_curve_command import request,DISPLAY_VERIFICATION
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_display_verification',request(list(DISPLAY_VERIFICATION)),[
  'generate_blend_curve_display_verification.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(name+'.py'for name in(module,*deps)),
 ],'VibocerosOracleBlendDisplayVerification20261107',timeout=1200)
