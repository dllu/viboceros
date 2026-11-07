"""Check existing analysis, private portions, surfaces and changed previews."""
from .capture_family import capture
from .blend_curve_command import request,DISPLAY_HOLDOUTS
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_display_holdouts',request(list(DISPLAY_HOLDOUTS)),[
  'generate_blend_curve_display_holdouts.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(name+'.py'for name in(module,*deps)),
 ],'VibocerosOracleBlendDisplayHoldouts20261107',timeout=1200)
