"""Observe Line construction modes in an owned empty document."""
if __package__:from .line_construction_cases import validate,recipe
else:from line_construction_cases import validate,recipe
def run(op,host,iterations,tolerance):
    validate(op)
    from line_scene_probe import run_case
    case=recipe(op['case']);doc=host['Rhino'].RhinoDoc.ActiveDoc;old_units=doc.ModelUnitSystem
    try:
        if 'model_units'in case:doc.ModelUnitSystem=getattr(host['Rhino'].UnitSystem,case['model_units'])
        value,elapsed=run_case(op,host,iterations,tolerance,case,'Line',False)
        if 'model_units'in case:value.update(model_units=str(doc.ModelUnitSystem),prescribed_case=case)
        return value,elapsed
    finally:doc.ModelUnitSystem=old_units
