"""Owned MatchSrf command picks, source identities, attributes and real history."""
if __package__:
    from .match_surface_command import CASES,validate
else:
    from match_surface_command import CASES,validate


def run(op,host,iterations,tolerance):
    validate(op)
    from line_scene_probe import run_case
    recipe=CASES[op['case']];R,S=host['Rhino'],host['System'];doc=R.RhinoDoc.ActiveDoc
    inputs=[];groups=[]
    def prepared(ids):
        inputs.extend(ids)
        for i,key in enumerate(ids):
            obj=doc.Objects.FindId(key);attrs=obj.Attributes.Duplicate()
            try:
                attrs.Name='match-source-'+str(i)
                attrs.ObjectColor=S.Drawing.Color.FromArgb(40+80*i,60,90)
                attrs.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
                if not recipe['common_group']:attrs.RemoveFromAllGroups()
                if not doc.Objects.ModifyAttributes(key,attrs,True):raise ValueError('MatchSrf source attributes failed')
            finally:attrs.Dispose()
            group=doc.Groups.Add('Match input group '+str(i),[key])
            if group<0:raise ValueError('MatchSrf source grouping failed')
            groups.append(group)
    def setup(ids):
        doc.Objects.UnselectAll()
        if recipe['preselected']:
            for index in recipe['selection_order']:doc.Objects.Select(ids[index])
    def observer(obj):return dict(input_index=inputs.index(obj.Id) if obj.Id in inputs else None)
    try:
        value,elapsed=run_case(op,host,iterations,tolerance,recipe,'MatchSrf',recipe.get('capture_recall',False),
            source_setup=setup,capture_undo=True,sources_prepared=prepared,object_observer=observer)
        value['prescribed_case']=recipe;return value,elapsed
    finally:
        for index in reversed(groups):doc.Groups.Delete(index)
