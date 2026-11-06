"""Capture native sliding and fixed perpendicular line workflows."""
from .capture_family import capture
from .line_perpendicular_cases import request
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['line_perpendicular']
 capture('line_perpendicular',request(),['generate_line_perpendicular.py','structural_sdk.py','rhino_worker.py','client.py','catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',*(n+'.py'for n in(module,*deps))],'VibocerosOracleLinePerpendicular20261106',timeout=1200)
