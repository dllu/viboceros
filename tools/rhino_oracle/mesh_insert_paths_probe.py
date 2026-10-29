"""Actual acknowledged InsertPoint getters for fixed authored mesh paths."""
try:
    from .mesh_insert_paths_cases import CASES,validate
except (ImportError,ValueError):
    from mesh_insert_paths_cases import CASES,validate

def run(op,host,iterations,tolerance):
    validate(op)
    from copy import deepcopy
    from crease_probe import run_scene
    case=deepcopy(CASES[op['case']]);case['capture_id']=op['id']
    return run_scene(case,host,iterations,tolerance)
