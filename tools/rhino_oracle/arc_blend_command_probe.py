"""Observe native ArcBlend geometry, input prompts, attributes and history."""
if __package__:from .arc_blend_command import CASES,validate
else:from arc_blend_command import CASES,validate
def run(op,host,iterations,tolerance):
 validate(op)
 from line_scene_probe import run_case
 recipe=CASES[op['case']];value,elapsed=run_case(op,host,iterations,tolerance,recipe,'ArcBlend',False);value['prescribed_case']=recipe
 return value,elapsed
