"""Capture polysurface base-face selection and IgnoreTrims reference scope."""
from .capture_family import capture
from .line_normal_cases import request
from .line_normal_brep import BREP
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['line_normal']
 capture('line_normal_brep',request(list(BREP)),['generate_line_normal_brep.py','structural_sdk.py','rhino_worker.py','client.py','catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',*(n+'.py'for n in(module,*deps))],'VibocerosOracleLineNormalBrep20261106',timeout=1200)
