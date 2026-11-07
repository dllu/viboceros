"""Read public BlendCrv output geometry, attributes, prompts and history."""
if __package__:from .blend_curve_command import CASES,validate
else:from blend_curve_command import CASES,validate
def observe(curve,host,jets=False,lengths=False):
 value=dict(samples=[dict(parameter=curve.Domain.ParameterAt(i/32.),point=host['_xyz'](curve.PointAt(curve.Domain.ParameterAt(i/32.))),tangent=host['_xyz'](curve.TangentAt(curve.Domain.ParameterAt(i/32.))))for i in range(33)])
 if jets:
  value['derivative_samples']=[dict(parameter=t,derivatives=[host['_xyz'](v)for v in curve.DerivativeAt(t,4)])for t in [curve.Domain.T0,curve.Domain.Mid,curve.Domain.T1]]
 if lengths:
  value['length_measurements']=dict(default=float(curve.GetLength()),fractional=[dict(tolerance=t,length=float(curve.GetLength(t)))for t in [1e-6,1e-8,1e-10,1e-12]])
  clone=curve.ToNurbsCurve()
  try:
   clone.Domain=host['Rhino'].Geometry.Interval(0.,1.)
   value['length_measurements']['unit_domain_fractional']=[dict(tolerance=t,length=float(clone.GetLength(t)))for t in [1e-6,1e-8,1e-10,1e-12]]
  finally:clone.Dispose()
 return value
def run(op,host,iterations,tolerance):
 validate(op)
 from line_scene_probe import run_case
 recipe=CASES[op['case']]
 def observation(curve):
  value=observe(curve,host,recipe.get('measure_jets',False),recipe.get('measure_lengths',False))
  if recipe.get('measure_curvature'):
   value['curvature_samples']=[dict(fraction=i/32.,point=host['_xyz'](curve.PointAt(curve.Domain.ParameterAt(i/32.))),curvature=host['_xyz'](curve.CurvatureAt(curve.Domain.ParameterAt(i/32.))))for i in range(33)]
  return value
 completion=None
 setup=None
 if recipe.get('display_completion'):
  from blend_curve_display_capture import completed
  completion=lambda:completed(host,op['id'],recipe.get('desktop_after',False))
 if 'initial_graphs'in recipe or recipe.get('reset_graphs'):
  from blend_curve_display_capture import setup as setup_graphs
  setup=lambda guides:setup_graphs(host,guides,recipe.get('initial_graphs',[]),recipe.get('reset_graphs',False))
 settings=host['Rhino'].ApplicationSettings.CurvatureGraphSettings
 old=(settings.HairScale,settings.HairDensity)if 'graph_settings'in recipe else None
 try:
  if 'graph_settings'in recipe:
   settings.HairScale=recipe['graph_settings']['scale'];settings.HairDensity=recipe['graph_settings']['density']
  value,elapsed=run_case(op,host,iterations,tolerance,recipe,'BlendCrv',False,observation,completion,setup);value['prescribed_case']=recipe
  return value,elapsed
 finally:
  if old is not None:settings.HairScale,settings.HairDensity=old
