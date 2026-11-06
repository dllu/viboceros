"""Capture physical tangent endpoint viewing rays in private Rhino sessions."""
from .capture_family import capture
from .line_tangent_rays import confirmation_request as request
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['line_tangent_rays']
 capture('line_tangent_mouse',request(),['generate_line_tangent_mouse.py','structural_sdk.py','rhino_worker.py','client.py','catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',*(n+'.py'for n in(module,*deps))],'VibocerosOracleLineTangentMouse20261106',timeout=1200)
