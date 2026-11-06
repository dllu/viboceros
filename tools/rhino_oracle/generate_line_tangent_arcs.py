"""Capture independent tangent arc boundaries and public SDK contacts."""
from .capture_family import capture
from .line_tangent_arcs import request
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['line_tangent_arcs']
 capture('line_tangent_arcs',request(),['generate_line_tangent_arcs.py','structural_sdk.py','rhino_worker.py','client.py','catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',*(n+'.py'for n in(module,*deps))],'VibocerosOracleLineTangentArcs20261106',timeout=1200)
