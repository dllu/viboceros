"""Capture degrees, charts, interior knots and orientation of surface merges."""
from .capture_family import capture
from .merge_surface_cases import HOLDOUTS, request


if __name__ == '__main__':
    capture('merge_surfaces_holdouts', request(list(HOLDOUTS)), [
        'generate_merge_surfaces_holdouts.py', 'merge_surface_cases.py', 'merge_surfaces_sdk.py',
        'structural_sdk.py', 'rhino_worker.py', 'client.py', 'brep_join_probe.py', 'cap_probe.py',
    ], 'VibocerosOracleMergeSurfacesHoldouts20261108', timeout=1200)
