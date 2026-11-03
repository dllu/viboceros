"""Public ExtractSrf command on owned SubD components, including real history."""
if __package__:
    from .subd_extract_cases import all_cases,validate
    from .subd_join_probe import source
else:
    from subd_extract_cases import all_cases,validate
    from subd_join_probe import source


def run(op,host,iterations,tolerance):
    validate(op)
    from subd_box_probe import subd_value
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or doc.UndoRecordingIsActive or R.Commands.Command.InCommand():
        raise ValueError('SubD extraction needs an empty owned idle document')
    recipe=all_cases()[op['scene']];ids=[];layers=[];groups=[]
    old_layer=doc.Layers.CurrentLayerIndex

    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            a=obj.Attributes
            rows.append(dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                selected=bool(obj.IsSelected(False)),layer=doc.Layers[a.LayerIndex].Name,name=a.Name,
                groups=[doc.Groups.GroupName(i) for i in (a.GetGroupList() or [])],
                color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],color_source=int(a.ColorSource),
                selected_faces=[i for i,f in enumerate(obj.Geometry.Faces) if obj.IsSubObjectSelected(f.ComponentIndex())],
                subd=subd_value(obj.Geometry,host)))
        return rows

    try:
        record=doc.BeginUndoRecord('Owned SubD extraction setup')
        try:
            for i,net in enumerate(recipe['sources']):
                layer=doc.Layers.Add('SubD extract source '+str(i),S.Drawing.Color.Blue);layers.append(layer)
                group=doc.Groups.Add('SubD extract group '+str(i));groups.append(group)
                a=R.DocObjects.ObjectAttributes();a.LayerIndex=layer;a.Name='Input '+str(i)
                a.ObjectColor=S.Drawing.Color.FromArgb(17+i,43+i,91+i);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject;a.AddToGroup(group)
                subd=source(net,host)
                try:ids.append(doc.Objects.AddSubD(subd,a))
                finally:subd.Dispose();a.Dispose()
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True)
        for owner,face in recipe['faces']:
            obj=doc.Objects.FindId(ids[owner]);f=list(obj.Geometry.Faces)[face]
            if not obj.SelectSubObject(f.ComponentIndex(),True,True,False):raise ValueError('SubD face selection failed')
        before=snapshot();marker='Owned SubD extraction '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        macro='_-ExtractSrf _Copy=_'+('Yes' if op['copy'] else 'No')+' _OutputLayer=_'+('Current' if recipe.get('current') else 'Input')+('' if recipe.get('default_corners') else ' _SubDAddCorners=_'+('Yes' if op['corners'] else 'No'))+' _Enter'
        success,after,events=observe_command(R.Commands.Command,'ExtractSrf',lambda:R.RhinoApp.RunScript(macro,False),snapshot,lambda:[],True)
        after_script=snapshot();depth=int(R.Commands.Command.InCommand())
        if depth:raise ValueError('extraction macro left an active command')
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(input=recipe,success=success,before=before,after=after,after_script=after_script,undo=undo,redo=redo,
                    history=history,results=[e['result'] for e in events if e['name']=='ExtractSrf']),0
    finally:
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        for group in groups:doc.Groups.Delete(group)
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
