"""Capture perpendicular guides with collapsed planar projection and corners."""
from .capture_family import capture
from .line_perpendicular_cases import request
from .line_perpendicular_boundaries import BOUNDARIES
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['line_perpendicular']
 capture('line_perpendicular_boundaries',request(list(BOUNDARIES)),['generate_line_perpendicular_boundaries.py','structural_sdk.py','rhino_worker.py','client.py','catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',*(n+'.py'for n in(module,*deps))],'VibocerosOracleLinePerpendicularBoundaries20261106',timeout=1200)
