"""Observe native tangent source and endpoint clicks in an owned viewport."""
if __package__:from .line_tangent_rays import CASES,CONFIRM,validate
else:from line_tangent_rays import CASES,CONFIRM,validate
def run(op,host,iterations,tolerance):
 validate(op)
 from line_scene_probe import run_case
 recipe=(CONFIRM if op['op']=='line_tangent_mouse'else CASES)[op['case']]
 value,elapsed=run_case(op,host,iterations,tolerance,recipe,'Line',False)
 value['prescribed_case']=recipe
 return value,elapsed
