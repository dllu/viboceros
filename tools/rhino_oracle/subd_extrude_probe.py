"""Public ExtrudeSubD command on owned input cages and selected components."""
if __package__:
    from .subd_extrude_cases import TARGETS,EXTRA_TARGETS,MIXED_TARGETS,validate
    from .subd_offset_cases import scenes
    from .subd_join_probe import source
else:
    from subd_extrude_cases import TARGETS,EXTRA_TARGETS,MIXED_TARGETS,validate
    from subd_offset_cases import scenes
    from subd_join_probe import source


def run(op,host,iterations,tolerance):
    validate(op)
    from subd_box_probe import subd_value
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];doc=R.RhinoDoc.ActiveDoc;key=None;layer=-1;group=-1
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    targets=dict(TARGETS);targets.update(EXTRA_TARGETS);targets.update(MIXED_TARGETS)
    scene,kind,indices=targets[op['target']]
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            a=obj.Attributes;g=obj.Geometry
            rows.append(dict(source=0 if obj.Id==key else None,selected=bool(obj.IsSelected(False)),name=a.Name,layer=doc.Layers[a.LayerIndex].Name,
                color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],color_source=int(a.ColorSource),
                groups=[doc.Groups.GroupName(i)for i in (a.GetGroupList()or[])],
                selected_faces=[i for i,f in enumerate(g.Faces)if obj.IsSubObjectSelected(f.ComponentIndex())],
                selected_edges=[i for i,e in enumerate(g.Edges)if obj.IsSubObjectSelected(e.ComponentIndex())],subd=subd_value(g,host)))
        return rows
    try:
        record=doc.BeginUndoRecord('Owned SubD extrusion setup')
        try:
            layer=doc.Layers.Add('SubD extrude source',S.Drawing.Color.Blue);group=doc.Groups.Add('SubD extrude group')
            a=R.DocObjects.ObjectAttributes();a.LayerIndex=layer;a.Name='Input';a.ObjectColor=S.Drawing.Color.FromArgb(17,43,91);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject;a.AddToGroup(group)
            subd=source(scenes()[scene],host)
            try:key=doc.Objects.AddSubD(subd,a)
            finally:subd.Dispose();a.Dispose()
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True);obj=doc.Objects.FindId(key)
        if kind=='whole':doc.Objects.Select(key)
        else:
            selections=(('face',indices['faces']),('edge',indices['edges']))if kind=='mixed' else ((kind,indices),)
            for component_kind,selected in selections:
                components=list(obj.Geometry.Faces if component_kind=='face' else obj.Geometry.Edges)
                for i in selected:
                    if not obj.SelectSubObject(components[i].ComponentIndex(),True,True,False):raise ValueError('SubD extrusion preselection failed')
        before=snapshot();mode=op['mode'];marker='Owned SubD extrusion '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        macro='_-ExtrudeSubD _Basis=_'+('WCS' if mode=='Z' else 'UVN')+' _Direction=_'+mode+' '+str(op['distance'])+' _Enter _Cancel _Cancel'
        success,after,events=observe_command(R.Commands.Command,'ExtrudeSubD',lambda:R.RhinoApp.RunScript(macro,False),snapshot,lambda:[],True)
        after_script=snapshot();depth=int(R.Commands.Command.InCommand())
        if depth:raise ValueError('ExtrudeSubD macro left an active command')
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(success=success,before=before,after=after,after_script=after_script,undo=undo,redo=redo,history=history,results=[e['result']for e in events if e['name']=='ExtrudeSubD']),0
    finally:
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        if group>=0:doc.Groups.Delete(group)
        if layer>=0:doc.Layers.Delete(layer,True)
