"""Capture public Shell getter and document behavior privately."""
from .capture_family import capture
from .shell_command import PILOT, request
from .structural_sdk import PROBES


if __name__ == '__main__':
    module, dependencies = PROBES['shell_command']
    capture('shell_getter_pilot', request(list(PILOT)), [
        'generate_shell_getter_pilot.py', 'client.py', 'rhino_worker.py',
        'structural_sdk.py', 'catenary_point_picker.py', 'arc_point_picker.py',
        'subcurve_direction_input.py', 'group_picking.py',
        'chamfer_point_picker.py', 'shell_point_picker.py',
        *(name + '.py' for name in (module, *dependencies)),
    ], 'VibocerosOracleShellGetterPilot20261108', timeout=1200)
