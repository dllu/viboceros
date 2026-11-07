"""Check symmetry copying and continuity resets of nondefault control nets."""
from .capture_family import capture
from .blend_curve_command import request,CONTROL_SYMMETRY
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_control_symmetry',request(list(CONTROL_SYMMETRY)),[
  'generate_blend_curve_control_symmetry.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(n+'.py'for n in(module,*deps)),
 ],'VibocerosOracleBlendControlSymmetry20261107',timeout=700)
