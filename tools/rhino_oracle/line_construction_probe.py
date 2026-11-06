"""Observe Line construction modes in an owned empty document."""
if __package__:from .line_construction_cases import validate,recipe
else:from line_construction_cases import validate,recipe
def run(op,host,iterations,tolerance):
    validate(op)
    from line_scene_probe import run_case
    return run_case(op,host,iterations,tolerance,recipe(op['case']),'Line',False)
