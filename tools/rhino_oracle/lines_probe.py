"""Observe the public Lines getter through the shared owned line scene."""
if __package__:from .lines_cases import validate,recipe
else:from lines_cases import validate,recipe

def run(op,host,iterations,tolerance):
    validate(op)
    from line_scene_probe import run_case
    return run_case(op,host,iterations,tolerance,recipe(op['case']),'Lines')
