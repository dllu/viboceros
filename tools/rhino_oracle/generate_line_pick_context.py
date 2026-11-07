"""Capture public picking contexts before physical tangent source clicks."""
from .capture_family import capture
from .line_pick_context import request
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['line_pick_context']
 capture('line_pick_context',request(),['generate_line_pick_context.py','structural_sdk.py','rhino_worker.py','client.py','catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',*(n+'.py'for n in(module,*deps))],'VibocerosOracleLinePickContext20261106',timeout=1200)
