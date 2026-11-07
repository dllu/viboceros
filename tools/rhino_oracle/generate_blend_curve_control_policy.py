"""Distinguish higher-jet policy and prompt-gated numeric secondary controls."""
from .capture_family import capture
from .blend_curve_command import request,CONTROL_POLICY
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_control_policy',request(list(CONTROL_POLICY)),[
  'generate_blend_curve_control_policy.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(n+'.py'for n in(module,*deps)),
 ],'VibocerosOracleBlendControlPolicy20261107',timeout=1200)
