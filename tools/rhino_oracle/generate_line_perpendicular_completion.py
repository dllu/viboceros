"""Capture real blank keys, Planar cursor behavior and Point override."""
from .capture_family import capture
from .line_perpendicular_cases import request
from .line_perpendicular_completion import COMPLETION
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['line_perpendicular']
 capture('line_perpendicular_completion',request(list(COMPLETION)),['generate_line_perpendicular_completion.py','structural_sdk.py','rhino_worker.py','client.py','catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',*(n+'.py'for n in(module,*deps))],'VibocerosOracleLinePerpendicularCompletion20261106',timeout=1200)
