"""Capture public BlendCrv temporary portions, directed ends and original parents."""
from .capture_family import capture
from .blend_curve_command import request,PORTION_HOLDOUTS
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_portion_holdouts',request(list(PORTION_HOLDOUTS)),[
  'generate_blend_curve_portion_holdouts.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(n+'.py'for n in(module,*deps)),
 ],'VibocerosOracleBlendCurvePortionHoldouts20261107',timeout=1600)
