"""Check distant control selection through fixed physical viewport clicks."""
from .capture_family import capture
from .blend_curve_command import request,ALT_PICKING
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_alt_picking',request(list(ALT_PICKING)),[
  'generate_blend_curve_alt_picking.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(n+'.py'for n in(module,*deps)),
 ],'VibocerosOracleBlendAltPicking20261107',timeout=700)
