"""Calibrate public Undo/Redo command event results before history qualification."""
from .capture_family import capture
from .fillet_command import ZERO_HISTORY_CONTROL, request
from .structural_sdk import PROBES

if __name__ == '__main__':
    module, deps = PROBES['fillet_command']
    capture('fillet_zero_history_control', request(list(ZERO_HISTORY_CONTROL)), [
        'generate_fillet_zero_history_control.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py',
        'catenary_point_picker.py', 'arc_point_picker.py', 'subcurve_direction_input.py', 'group_picking.py',
        *(name+'.py' for name in (module,*deps)),
    ], 'VibocerosOracleFilletZeroHistoryControl20261108', timeout=1200)
