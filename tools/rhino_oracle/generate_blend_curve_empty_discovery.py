"""Capture endpoint flips and zero retained intervals without desktop windows."""
from .capture_family import capture
from .blend_curve_command import request,EMPTY_DISCOVERY
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_empty_discovery',request(list(EMPTY_DISCOVERY)),[
  'generate_blend_curve_empty_discovery.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(n+'.py'for n in(module,*deps)),
 ],'VibocerosOracleBlendEmptyDiscovery20261107',timeout=1200)
