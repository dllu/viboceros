"""Capture native ArcBlend adjustment handles in a private desktop."""
from .capture_family import capture
from .arc_blend_command import request, CHORD_HANDLES
from .structural_sdk import PROBES

if __name__ == '__main__':
    module, deps = PROBES['arc_blend_command']
    capture('arc_blend_chord_handles', request(list(CHORD_HANDLES)), [
        'generate_arc_blend_chord_handles.py', 'structural_sdk.py', 'rhino_worker.py',
        'client.py', 'catenary_point_picker.py', 'arc_point_picker.py',
        'subcurve_direction_input.py', 'group_picking.py',
        *(name + '.py' for name in (module, *deps)),
    ], 'VibocerosOracleArcBlendChordHandles20261106', timeout=1200)
