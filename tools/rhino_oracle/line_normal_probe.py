"""Observe Line Normal in an owned empty scene and private viewport."""
if __package__:from .line_normal_cases import validate,recipe
else:from line_normal_cases import validate,recipe
def run(op,host,iterations,tolerance):
 validate(op)
 from line_scene_probe import run_case
 return run_case(op,host,iterations,tolerance,recipe(op['case']),'Line',False)
