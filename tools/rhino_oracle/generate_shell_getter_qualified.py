"""Capture physical Shell face picks and bounded getter controls privately."""
from .capture_family import capture
from .shell_command import QUALIFIED, request
from .structural_sdk import PROBES


if __name__ == '__main__':
    module, dependencies = PROBES['shell_command']
    capture('shell_getter_qualified', request(list(QUALIFIED)), [
        'generate_shell_getter_qualified.py', 'client.py', 'rhino_worker.py',
        'structural_sdk.py', 'catenary_point_picker.py', 'arc_point_picker.py',
        'subcurve_direction_input.py', 'group_picking.py',
        'chamfer_point_picker.py', 'shell_point_picker.py',
        *(name + '.py' for name in (module, *dependencies)),
    ], 'VibocerosOracleShellGetterQualified20261108', timeout=1200)
