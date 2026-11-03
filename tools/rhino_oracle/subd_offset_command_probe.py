"""Named OffsetSubD events and native object/history policy in an owned model."""
if __package__:
    from .subd_offset_cases import scenes
    from .subd_offset_command_cases import validate
    from .subd_join_probe import source
else:
    from subd_offset_cases import scenes
    from subd_offset_command_cases import validate
    from subd_join_probe import source


def run(op,host,iterations,tolerance):
    validate(op)
    return run_case(op,host,iterations,tolerance)


def run_case(op,host,iterations,tolerance):
    from subd_box_probe import subd_value
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];doc=R.RhinoDoc.ActiveDoc;keys=[];layers=[];group=None;old_layer=doc.Layers.CurrentLayerIndex
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            a=obj.Attributes
            rows.append(dict(source=keys.index(obj.Id) if obj.Id in keys else None,selected=bool(obj.IsSelected(False)),
                name=a.Name,layer=doc.Layers[a.LayerIndex].Name,
                groups=[doc.Groups.GroupName(i) for i in (a.GetGroupList() or [])],
                color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],color_source=int(a.ColorSource),subd=subd_value(obj.Geometry,host)))
        return rows
    try:
        record=doc.BeginUndoRecord('Owned SubD offset setup')
        try:
            for name in ('source','current'):layers.append(doc.Layers.Add('SubD offset '+name,S.Drawing.Color.Blue))
            doc.Layers.SetCurrentLayerIndex(layers[1],True);group=doc.Groups.Add('SubD offset group')
            nets=op.get('many',[op['scene']])
            for i,scene in enumerate(nets):
                a=R.DocObjects.ObjectAttributes();a.LayerIndex=layers[0];a.Name='Input' if len(nets)==1 else 'Input '+str(i);a.ObjectColor=S.Drawing.Color.FromArgb(17+i,43+i,91+i);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject;a.AddToGroup(group)
                subd=source(scenes()[scene],host)
                try:keys.append(doc.Objects.AddSubD(subd,a))
                finally:subd.Dispose();a.Dispose()
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True)
        if op['preselect']:
            for key in keys:doc.Objects.Select(key)
        before=snapshot();marker='Owned SubD offset '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        macro='_-OffsetSubD '+('' if op['preselect'] else ' '.join('_SelID '+str(key) for key in keys)+' _Enter ')
        if not op.get('defaults'):
            macro+=' '.join('_'+name+'=_'+('Yes' if op[field] else 'No')for name,field in [('Solid','solid'),('BothSides','both_sides'),('DeleteInput','delete_input')])
        macro+=(' _FlipAll' if op['flip'] else '')+('' if op.get('defaults') else ' '+str(op['distance']))+(' _Cancel' if op.get('cancel') else ' _Enter')+' _Cancel _Cancel'
        success,after,events=observe_command(R.Commands.Command,'OffsetSubD',lambda:R.RhinoApp.RunScript(macro,False),snapshot,lambda:[],True)
        after_script=snapshot();depth=int(R.Commands.Command.InCommand())
        if depth:raise ValueError('OffsetSubD macro left an active command')
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(success=success,before=before,after=after,after_script=after_script,undo=undo,redo=redo,history=history,
                    results=[e['result']for e in events if e['name']=='OffsetSubD']),0
    finally:
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        if group is not None:doc.Groups.Delete(group)
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
