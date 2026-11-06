"""Capture independent Line Normal curve and surface constructions."""
from .capture_family import capture
from .line_normal_cases import request
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['line_normal']
 capture('line_normal',request(),['generate_line_normal.py','structural_sdk.py','rhino_worker.py','client.py','catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',*(n+'.py'for n in(module,*deps))],'VibocerosOracleLineNormal20261106',timeout=1200)
