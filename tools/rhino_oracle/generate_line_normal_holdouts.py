"""Capture independent Normal completion, straight and trimmed references."""
from .capture_family import capture
from .line_normal_cases import request
from .line_normal_holdouts import HOLDOUTS
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['line_normal']
 capture('line_normal_holdouts',request(list(HOLDOUTS)),['generate_line_normal_holdouts.py','structural_sdk.py','rhino_worker.py','client.py','catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',*(n+'.py'for n in(module,*deps))],'VibocerosOracleLineNormalHoldouts20261106',timeout=1200)
