"""Capture public BlendCrv Shift handles and contact symmetry."""
from .capture_family import capture
from .blend_curve_command import request,SYMMETRY_LENGTHS
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['blend_curve_command']
 capture('blend_curve_symmetry_lengths',request(list(SYMMETRY_LENGTHS)),[
  'generate_blend_curve_symmetry_lengths.py','structural_sdk.py','rhino_worker.py','client.py',
  'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
  *(n+'.py'for n in(module,*deps)),
 ],'VibocerosOracleBlendCurveSymmetryLengths20261107',timeout=1600)
