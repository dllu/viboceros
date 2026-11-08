"""Capture authored public surface merges inside the headless oracle wrapper."""
from .capture_family import capture
from .merge_surface_cases import PILOT, request


if __name__ == '__main__':
    capture('merge_surfaces_pilot', request(list(PILOT)), [
        'generate_merge_surfaces_pilot.py', 'merge_surface_cases.py', 'merge_surfaces_sdk.py',
        'structural_sdk.py', 'rhino_worker.py', 'client.py', 'brep_join_probe.py', 'cap_probe.py',
    ], 'VibocerosOracleMergeSurfacesPilot20261108', timeout=1200)
