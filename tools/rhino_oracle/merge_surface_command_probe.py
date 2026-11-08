"""Public MergeSrf getters with owned inputs, attributes and actual history."""
if __package__:
    from .merge_surface_command import CASES, validate
else:
    from merge_surface_command import CASES, validate


def run(op, host, iterations, tolerance):
    validate(op)
    from line_scene_probe import run_case
    recipe = CASES[op['case']]
    R, S = host['Rhino'], host['System']
    doc = R.RhinoDoc.ActiveDoc
    inputs, groups = [], []

    def prepared(ids):
        inputs.extend(ids)
        for i, key in enumerate(ids):
            obj = doc.Objects.FindId(key)
            attrs = obj.Attributes.Duplicate()
            try:
                attrs.Name = 'merge-source-' + str(i)
                attrs.ObjectColor = S.Drawing.Color.FromArgb(40 + 80 * i, 60, 90)
                attrs.ColorSource = R.DocObjects.ObjectColorSource.ColorFromObject
                if not recipe.get('common_group', True): attrs.RemoveFromAllGroups()
                if not doc.Objects.ModifyAttributes(key, attrs, True):
                    raise ValueError('owned MergeSrf attributes failed')
            finally:
                attrs.Dispose()
            group = doc.Groups.Add('Merge input group ' + str(i), [key])
            if group < 0: raise ValueError('owned MergeSrf grouping failed')
            groups.append(group)

    def setup(ids):
        doc.Objects.UnselectAll()
        if recipe['preselected']:
            for index in recipe['selection_order']: doc.Objects.Select(ids[index])

    def observer(obj):
        return dict(input_index=inputs.index(obj.Id) if obj.Id in inputs else None)

    try:
        value, elapsed = run_case(op, host, iterations, tolerance, recipe, 'MergeSrf', recipe.get('capture_recall', False),
            source_setup=setup, capture_undo=True, sources_prepared=prepared,
            object_observer=observer)
        value['prescribed_case'] = recipe
        return value, elapsed
    finally:
        for index in reversed(groups): doc.Groups.Delete(index)
