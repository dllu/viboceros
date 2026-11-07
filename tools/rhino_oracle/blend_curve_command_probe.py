"""Read public BlendCrv output geometry, attributes, prompts and history."""
if __package__:from .blend_curve_command import CASES,validate
else:from blend_curve_command import CASES,validate
def observe(curve,host):
 return dict(samples=[dict(parameter=curve.Domain.ParameterAt(i/32.),point=host['_xyz'](curve.PointAt(curve.Domain.ParameterAt(i/32.))),tangent=host['_xyz'](curve.TangentAt(curve.Domain.ParameterAt(i/32.))))for i in range(33)])
def run(op,host,iterations,tolerance):
 validate(op)
 from line_scene_probe import run_case
 recipe=CASES[op['case']]
 value,elapsed=run_case(op,host,iterations,tolerance,recipe,'BlendCrv',False,lambda c:observe(c,host));value['prescribed_case']=recipe
 return value,elapsed
