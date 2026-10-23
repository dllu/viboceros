"""Saved source topology and command/history observations for InsertPoint."""
if __package__:
    from .insert_point_cases import ALL_CASES,validate
    from .crease_probe import run_scene
else:
    from insert_point_cases import ALL_CASES,validate
    from crease_probe import run_scene
def run(op,host,iterations,tolerance):
    validate(op)
    from copy import deepcopy
    case=deepcopy(ALL_CASES[op['case']]);case['capture_id']=op['id']
    return run_scene(case,host,iterations,tolerance)
