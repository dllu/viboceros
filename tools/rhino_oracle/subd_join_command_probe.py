"""Owned Join/JoinCopy command policy, separate from the SubD joining SDK."""
if __package__:
    from .subd_join_cases import case
    from .subd_join_probe import source
else:
    from subd_join_cases import case
    from subd_join_probe import source

SCENES=('single','adjacent','disjoint','point','chain','duplicate','threeway','gap_half')


def validate(op):
    if (not isinstance(op,dict) or set(op)!={'op','id','scene','command','preselect','crease'}
            or op['op'] not in ('subd_join_command','subd_join_history') or op['scene'] not in SCENES
            or op['command'] not in ('Join','JoinCopy')
            or not isinstance(op['preselect'],bool) or not isinstance(op['crease'],bool)
            or op['id']!='-'.join((op['scene'],op['command'],'pre' if op['preselect'] else 'post','crease' if op['crease'] else 'smooth'))):
        raise ValueError('fixed SubD join command recipe required')


def run(op,host,iterations,tolerance):
    validate(op)
    from subd_box_probe import subd_value
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or doc.UndoRecordingIsActive or R.Commands.Command.InCommand():
        raise ValueError('SubD join command requires an empty owned idle document')
    recipe=case(op['scene'],op['crease']);ids=[];layers=[];groups=[]
    old_layer=doc.Layers.CurrentLayerIndex;old_tolerance=doc.ModelAbsoluteTolerance

    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            a=obj.Attributes
            rows.append(dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                selected=bool(obj.IsSelected(False)),layer=doc.Layers[a.LayerIndex].Name,name=a.Name,
                groups=[doc.Groups.GroupName(i) for i in (a.GetGroupList() or [])],
                color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],color_source=int(a.ColorSource),
                subd=subd_value(obj.Geometry,host)))
        return rows

    try:
        doc.ModelAbsoluteTolerance=recipe['tolerance']
        # Seed the remembered option on independent owned objects; measured inputs
        # are never reused as a preference-setting transaction.
        option='_SubDJoinedEdges=_'+('Crease' if op['crease'] else 'Smooth')
        seeds=[]
        try:
            for d in case('adjacent')['sources']:
                subd=source(d,host)
                try:seeds.append(doc.Objects.AddSubD(subd))
                finally:subd.Dispose()
            macro='_-'+op['command']+' '+option+' '+' '.join('_SelID '+str(k) for k in seeds)+' _Enter _Cancel _Cancel'
            R.RhinoApp.RunScript(macro,False)
        finally:
            for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        undo=doc.BeginUndoRecord('Owned SubD join setup')
        try:
            for i,d in enumerate(recipe['sources']):
                layer=doc.Layers.Add('SubD join source '+str(i),S.Drawing.Color.Blue);layers.append(layer)
                group=doc.Groups.Add('SubD join group '+str(i));groups.append(group)
                a=R.DocObjects.ObjectAttributes();a.LayerIndex=layer;a.Name='Input '+str(i)
                a.ObjectColor=S.Drawing.Color.FromArgb(17+i,43+i,91+i);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject;a.AddToGroup(group)
                subd=source(d,host)
                try:ids.append(doc.Objects.AddSubD(subd,a))
                finally:subd.Dispose();a.Dispose()
        finally:doc.EndUndoRecord(undo)
        doc.ClearUndoRecords(True)
        if op['preselect']:
            for key in ids:doc.Objects.Select(key)
        before=snapshot()
        if op['op']=='subd_join_history':
            macro='_-'+op['command']+('' if op['preselect'] else ' '+option+' '+' '.join('_SelID '+str(k) for k in ids))+' _Enter'
        else:
            macro='_-'+op['command']+' '+('' if op['preselect'] else option+' '+' '.join('_SelID '+str(k) for k in ids))+' _Enter _Cancel _Cancel'
        success,after,events=observe_command(R.Commands.Command,op['command'],lambda:R.RhinoApp.RunScript(macro,False),snapshot,lambda:[],True)
        after_script=snapshot()
        command_depth=int(R.Commands.Command.InCommand())
        if op['op']=='subd_join_history' and command_depth:
            raise ValueError('history macro left an active command: '+str(command_depth))
        R.RhinoApp.RunScript('_Undo',False);undone=snapshot();R.RhinoApp.RunScript('_Redo',False);redone=snapshot()
        return dict(input=recipe,success=success,before=before,after=after,after_script=after_script,command_depth=command_depth,undo=undone,redo=redone,
                    results=[e['result'] for e in events if e['name']==op['command']]),0
    finally:
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        for group in groups:doc.Groups.Delete(group)
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
        doc.ModelAbsoluteTolerance=old_tolerance
