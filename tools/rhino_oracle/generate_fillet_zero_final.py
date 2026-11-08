"""Repeat every zero-radius workflow with direct public Undo/Redo observations."""
from .capture_family import capture
from .fillet_command import ZERO_FINAL, request
from .structural_sdk import PROBES

if __name__ == '__main__':
    module, deps = PROBES['fillet_command']
    capture('fillet_zero_final', request(list(ZERO_FINAL)), [
        'generate_fillet_zero_final.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py',
        'catenary_point_picker.py', 'arc_point_picker.py', 'subcurve_direction_input.py', 'group_picking.py',
        *(name+'.py' for name in (module,*deps)),
    ], 'VibocerosOracleFilletZeroFinal20261108', timeout=2200)
