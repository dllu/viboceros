"""Control observations verify distant selection independently of Alt."""
from .capture_family import capture
from .blend_curve_command import request,ALT_SELECTION
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_alt_selection',request(list(ALT_SELECTION)),[
  'generate_blend_curve_alt_selection.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(n+'.py'for n in(module,*deps)),
 ],'VibocerosOracleBlendPlainSelection20261107',timeout=700)
