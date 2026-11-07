"""Repeat every failed-pilot recipe in a new bounded private session."""
from .capture_family import capture
from .blend_curve_command import request,CLOSED_VERIFICATION
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_closed_verification',request(list(CLOSED_VERIFICATION)),[
  'generate_blend_curve_closed_verification.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(n+'.py'for n in(module,*deps)),
 ],'VibocerosOracleBlendClosedVerification20261107',timeout=900)
