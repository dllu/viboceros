"""Record completed Plane workflows with camera and stable cursor telemetry."""
from .capture_family import capture
from .plane_surface_cases import request, COMPLETION_CASES

if __name__ == '__main__':
    capture('plane_completion_terminal', request(list(COMPLETION_CASES)),
            ['generate_plane_completion_terminal.py','plane_surface_probe.py',
             'structural_sdk.py','rhino_worker.py','client.py'],
            'VibocerosOraclePlaneCompletionTerminal20261106',timeout=1000)
