"""Observe OffsetSrf through public command, document and geometry APIs."""
if __package__:
    from .offset_surface_command import CASES, validate
else:
    from offset_surface_command import CASES, validate

def run(op,host,iterations,tolerance):
    validate(op)
    from line_scene_probe import run_case
    recipe=CASES[op['case']];R=host['Rhino'];doc=R.RhinoDoc.ActiveDoc
    def setup(ids):
        doc.Objects.UnselectAll()
        if recipe.get('preselected'):
            for id in ids:doc.Objects.Select(id)
    value,elapsed=run_case(op,host,iterations,tolerance,recipe,'OffsetSrf',False,source_setup=setup,capture_undo=True)
    value['prescribed_case']=recipe
    return value,elapsed
