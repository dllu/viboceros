"""Capture first-side, input-order and face-sense surface merging variants."""
from .capture_family import capture
from .merge_surface_orientation_cases import request


if __name__ == '__main__':
    capture('merge_surface_orientation', request(), [
        'generate_merge_surface_orientation.py', 'merge_surface_orientation_cases.py',
        'merge_surface_orientation_sdk.py', 'merge_surface_cases.py', 'structural_sdk.py',
        'rhino_worker.py', 'client.py', 'brep_join_probe.py', 'cap_probe.py',
    ], 'VibocerosOracleMergeSurfaceOrientation20261108', timeout=1200)
