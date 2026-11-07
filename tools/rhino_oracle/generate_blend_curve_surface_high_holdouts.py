"""Capture curved-face G3/G4 edits, charts, joins and fourth-order diagnostics."""
from .capture_family import capture
from .blend_curve_command import request,SURFACE_HIGH_HOLDOUTS
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_surface_high_holdouts',request(list(SURFACE_HIGH_HOLDOUTS)),[
  'generate_blend_curve_surface_high_holdouts.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(n+'.py'for n in(module,*deps)),
 ],'VibocerosOracleSurfaceHighHoldoutsB20261107',timeout=2400)
