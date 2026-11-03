"""Actual ToNURBS commands, typed options, identities and undo in an owned model."""
if __package__:
    from .subd_nurbs_command_cases import validate,MODES
    from .subd_packing_cases import source
    from .subd_to_nurbs_probe import make_source
    from .subd_box_probe import subd_value
    from .join_probe import observe_command
else:
    from subd_nurbs_command_cases import validate,MODES
    from subd_packing_cases import source
    from subd_to_nurbs_probe import make_source
    from subd_box_probe import subd_value
    from join_probe import observe_command

def run(op,host,iterations,tolerance):
    validate(op);R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or doc.UndoRecordingIsActive or R.Commands.Command.InCommand():
        raise ValueError('empty private idle document required')
    layers=[];groups=[];keys=[];old_layer=doc.Layers.CurrentLayerIndex;results=[]
    def snapshot(geometry=False):
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            a=obj.Attributes;g=obj.Geometry
            row=dict(source=keys.index(obj.Id) if obj.Id in keys else None,selected=bool(obj.IsSelected(False)),
                name=a.Name,layer=layers.index(a.LayerIndex),groups=[groups.index(i) for i in (a.GetGroupList() or [])],
                color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],color_source=int(a.ColorSource),
                kind=g.GetType().Name,crc=int(g.DataCRC(0)))
            if geometry and isinstance(g,G.Brep):row['brep']=host['_interchange_brep_record'](g)
            if geometry and isinstance(g,G.SubD):row['subd']=subd_value(g,host)
            rows.append(row)
        return dict(objects=rows,groups=[dict(name=doc.Groups.GroupName(i),members=[j for j,o in enumerate(rows) if n in o['groups']]) for n,i in enumerate(groups)])
    try:
        for name in ('source','current'):layers.append(doc.Layers.Add('ToNURBS '+name,S.Drawing.Color.Blue))
        doc.Layers.SetCurrentLayerIndex(layers[1],True)
        for name in ('all','first','empty'):groups.append(doc.Groups.Add('ToNURBS '+name))
        for step in op['steps']:
            for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
            keys[:]=[];subd=make_source(source('grid' if op['scene']=='mixed' else op['scene']),host)
            try:
                if op['scene']=='subdivided_box' and not subd.Subdivide(1):raise ValueError('cannot subdivide owned source')
                nets=[subd]
                if op['scene']=='mixed':
                    nets += [G.Point(G.Point3d(20,0,0)),G.LineCurve(G.Point3d(10,0,0),G.Point3d(14,2,0)),
                        G.NurbsCurve.Create(False,1,[G.Point3d(30,0,0),G.Point3d(34,1,0)])]
                if step.get('mesh'):
                    mesh=G.Mesh();mesh.Vertices.Add(40,0,0);mesh.Vertices.Add(44,0,0);mesh.Vertices.Add(40,3,0)
                    mesh.Faces.AddFace(0,1,2);mesh.Normals.ComputeNormals();nets.append(mesh)
                try:
                    for i,net in enumerate(nets):
                        a=R.DocObjects.ObjectAttributes();a.LayerIndex=layers[0];a.Name='Input '+str(i)
                        a.ObjectColor=S.Drawing.Color.FromArgb(17+i,43+i,91+i);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
                        a.AddToGroup(groups[0])
                        if i==0:a.AddToGroup(groups[1])
                        try:keys.append(doc.Objects.Add(net,a))
                        finally:a.Dispose()
                        if keys[-1]==S.Guid.Empty:raise ValueError('cannot insert owned source')
                    model=R.FileIO.File3dm()
                    try:model.Objects.AddSubD(subd);archive=[int(b) for b in model.ToByteArray()]
                    finally:model.Dispose()
                finally:
                    for net in nets[1:]:net.Dispose()
            finally:subd.Dispose()
            doc.ClearUndoRecords(True);doc.Objects.UnselectAll()
            selected=list(reversed(range(len(keys)))) if step.get('postselect') else list(range(len(keys)))
            if not step.get('postselect'):
                for i in selected:doc.Objects.Select(keys[i])
            before=snapshot(True);marker='Owned SubD ToNURBS '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
            macro='_-ToNURBS'
            if step.get('postselect'):
                macro+=' '+' '.join('_SelID '+str(keys[i]) for i in selected if op['scene']!='mixed' or i!=1)
                macro+=' !' if step.get('cancel')=='selection' else ' _Enter'
            if step.get('cancel')!='selection':
                if 'delete' in step:macro+=' _DeleteInputObjects=_'+('Yes' if step['delete'] else 'No')
                if 'packed' in step or 'mode' in step:
                    macro+=' _SubDOptions'
                    if 'packed' in step:macro+=' _Faces=_'+('Packed' if step['packed'] else 'Unpacked')
                    if 'mode' in step:macro+=' _ExtraordinaryVertex=_'+step['mode']
                    macro+=' !' if step.get('cancel')=='submenu' else ' _Enter'
                if step.get('cancel')!='submenu':macro+=' !' if step.get('cancel')=='options' else ' _Enter'
            success,after,events=observe_command(R.Commands.Command,'ToNURBS',lambda:R.RhinoApp.RunScript(macro,True),lambda:snapshot(True),lambda:[],True)
            if R.Commands.Command.InCommand():raise ValueError('ToNURBS macro left a command active')
            after_script=snapshot()
            history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
            if step.get('cancel'):undo=redo=None
            else:
                R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
            results.append(dict(options=step,macro=macro,success=success,before=before,after=after,after_script=after_script,undo=undo,redo=redo,
                archive=archive,history=history,results=[e['result'] for e in events if e['name']=='ToNURBS']))
        return dict(steps=results),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        for group in reversed(groups):doc.Groups.Delete(group)
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
