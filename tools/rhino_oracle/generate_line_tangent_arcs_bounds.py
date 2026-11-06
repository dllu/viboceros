"""Capture dense local tangent queries through the public RhinoCommon API."""
from .capture_family import capture
from .line_tangent_arcs_sdk import request,BOUNDS
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['line_tangent_arcs_sdk']
 capture('line_tangent_arcs_bounds',request(list(BOUNDS)),['generate_line_tangent_arcs_bounds.py','structural_sdk.py','rhino_worker.py','client.py',*(n+'.py'for n in(module,*deps))],'VibocerosOracleLineTangentArcsBounds20261106',timeout=1200)
