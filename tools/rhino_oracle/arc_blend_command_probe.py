"""Observe native ArcBlend geometry, input prompts, attributes and history."""
if __package__:from .arc_blend_command import CASES,validate
else:from arc_blend_command import CASES,validate
def run(op,host,iterations,tolerance):
 validate(op)
 from line_scene_probe import run_case
 recipe=CASES[op['case']];observer=None
 if recipe.get('measure_arcs'):
  from arc_blend_measure import observe
  observer=lambda curve:observe(curve,host)
 doc=host['Rhino'].RhinoDoc.ActiveDoc;old_units=doc.ModelUnitSystem
 try:
  if 'model_units'in recipe:doc.ModelUnitSystem=getattr(host['Rhino'].UnitSystem,recipe['model_units'])
  value,elapsed=run_case(op,host,iterations,tolerance,recipe,'ArcBlend',False,observer);value['prescribed_case']=recipe
  if 'model_units'in recipe:value['model_units']=str(doc.ModelUnitSystem)
  return value,elapsed
 finally:doc.ModelUnitSystem=old_units
