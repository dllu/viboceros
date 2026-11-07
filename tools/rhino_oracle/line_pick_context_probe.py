"""Observe independently prescribed source picks and fixed tangent constructions."""
if __package__:from .line_pick_context import CASES,validate
else:from line_pick_context import CASES,validate
def run(op,host,iterations,tolerance):
 validate(op)
 from line_scene_probe import run_case
 recipe=CASES[op['case']];value,elapsed=run_case(op,host,iterations,tolerance,recipe,'Line',False)
 value['prescribed_case']=recipe
 return value,elapsed
