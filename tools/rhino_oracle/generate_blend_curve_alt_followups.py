"""Capture corrected angle-handle picking, spatial faces and mixed endpoints."""
from .capture_family import capture
from .blend_curve_command import request,ALT_FOLLOWUPS
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_alt_followups',request(list(ALT_FOLLOWUPS)),[
  'generate_blend_curve_alt_followups.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(n+'.py'for n in(module,*deps)),
 ],'VibocerosOracleBlendAltFollowups20261107',timeout=1200)
