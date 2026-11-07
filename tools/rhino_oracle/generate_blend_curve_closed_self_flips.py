"""Disambiguate two blend references to the same closed source."""
from .capture_family import capture
from .blend_curve_command import request,CLOSED_SELF_FLIPS
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_closed_self_flips',request(list(CLOSED_SELF_FLIPS)),[
  'generate_blend_curve_closed_self_flips.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(n+'.py'for n in(module,*deps)),
 ],'VibocerosOracleBlendClosedSelfFlips20261107',timeout=500)
