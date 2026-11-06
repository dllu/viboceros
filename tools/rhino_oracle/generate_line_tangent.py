"""Capture independent native tangent-line point and curve workflows."""
from .capture_family import capture
from .line_tangent_cases import request
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['line_tangent']
 capture('line_tangent',request(),['generate_line_tangent.py','structural_sdk.py','rhino_worker.py','client.py','catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',*(n+'.py'for n in(module,*deps))],'VibocerosOracleLineTangent20261106',timeout=1200)
