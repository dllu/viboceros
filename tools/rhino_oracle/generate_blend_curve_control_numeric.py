"""Measure higher-control numeric origins after spacing changes."""
from .capture_family import capture
from .blend_curve_command import request,CONTROL_NUMERIC
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_control_numeric',request(list(CONTROL_NUMERIC)),[
  'generate_blend_curve_control_numeric.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(n+'.py'for n in(module,*deps)),
 ],'VibocerosOracleBlendControlNumeric20261107',timeout=1000)
