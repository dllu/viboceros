"""Capture Shell option defaults, side openings and two-object batches."""
from .capture_family import capture
from .shell_command import HOLDOUTS, request
from .structural_sdk import PROBES


if __name__ == '__main__':
    module, dependencies = PROBES['shell_command']
    capture('shell_getter_holdouts', request(list(HOLDOUTS)), [
        'generate_shell_getter_holdouts.py', 'client.py', 'rhino_worker.py',
        'structural_sdk.py', 'catenary_point_picker.py', 'arc_point_picker.py',
        'subcurve_direction_input.py', 'group_picking.py',
        'chamfer_point_picker.py', 'shell_point_picker.py',
        *(name + '.py' for name in (module, *dependencies)),
    ], 'VibocerosOracleShellGetterHoldouts20261108', timeout=1200)
