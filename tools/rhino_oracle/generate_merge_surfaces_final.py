"""Capture gauge invariance and unequal degrees in inherited nonunit charts."""
from .capture_family import capture
from .merge_surface_cases import FINAL, request


if __name__ == '__main__':
    capture('merge_surfaces_final', request(list(FINAL)), [
        'generate_merge_surfaces_final.py', 'merge_surface_cases.py', 'merge_surfaces_sdk.py',
        'structural_sdk.py', 'rhino_worker.py', 'client.py', 'brep_join_probe.py', 'cap_probe.py',
    ], 'VibocerosOracleMergeSurfacesFinal20261108', timeout=1200)
