"""Observe nonlinear contacts, source edits and native scalar quantities."""
from .capture_family import capture
from .arc_blend_command import request,CURVED_SOURCES,LENGTH_INPUT
from .structural_sdk import PROBES

if __name__=='__main__':
    module,deps=PROBES['arc_blend_command']
    capture('arc_blend_sources',request([*CURVED_SOURCES,*LENGTH_INPUT]),[
        'generate_arc_blend_sources.py','structural_sdk.py','rhino_worker.py',
        'client.py','catenary_point_picker.py','arc_point_picker.py',
        'subcurve_direction_input.py','group_picking.py',
        *(name+'.py'for name in(module,*deps)),
    ],'VibocerosOracleArcBlendSourcesRetry20261106',timeout=1500)
