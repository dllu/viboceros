"""Capture the transient curvature toggle and public viewport pixels."""
from .capture_family import capture
from .blend_curve_command import request,DISPLAY_DISCOVERY
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_display_discovery',request(list(DISPLAY_DISCOVERY)),[
  'generate_blend_curve_display_discovery.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(name+'.py'for name in(module,*deps)),
 ],'VibocerosOracleBlendDisplayDiscovery20261107',timeout=1200)
