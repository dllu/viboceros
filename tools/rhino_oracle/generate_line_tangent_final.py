"""Capture tangent retries and drafting precedence."""
from .capture_family import capture
from .line_tangent_cases import request
from .line_tangent_final import FINAL
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['line_tangent']
 capture('line_tangent_final',request(list(FINAL)),['generate_line_tangent_final.py','structural_sdk.py','rhino_worker.py','client.py','catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',*(n+'.py'for n in(module,*deps))],'VibocerosOracleLineTangentFinal20261106',timeout=1200)
