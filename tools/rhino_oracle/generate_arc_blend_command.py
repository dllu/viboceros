"""Capture native ArcBlend command picks without desktop windows."""
from .capture_family import capture
from .arc_blend_command import request
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['arc_blend_command']
 capture('arc_blend_command',request(),['generate_arc_blend_command.py','structural_sdk.py','rhino_worker.py','client.py','catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',*(n+'.py'for n in(module,*deps))],'VibocerosOracleArcBlendCommand20261106',timeout=1200)
