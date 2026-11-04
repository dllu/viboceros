"""Owned PackSubDFaces commands: retained charts, identities and history."""
if __package__:
    from .subd_packing_cases import source
    from .subd_to_nurbs_probe import make_source
    from .join_probe import observe_command
else:
    from subd_packing_cases import source
    from subd_to_nurbs_probe import make_source
    from join_probe import observe_command

SCENES=('grid','grid_rotated','grid_shuffled','grid_L','grid_hole','grid_T',
        'cylinder','torus','box','subdivided_box','pentagonal_prism','five_fan')

def cases():
    rows=[dict(id=scene,scene=scene) for scene in SCENES]
    rows += [dict(id='stale_'+str(post),scene='grid',stale=True,postselect=post) for post in (False,True)]
    rows += [dict(id='mixed_'+str(post),scene='mixed',postselect=post) for post in (False,True)]
    rows.append(dict(id='cancel',scene='grid',postselect=True,cancel=True))
    return [dict(row,op='subd_pack_command') for row in rows]

def validate(op):
    if (not isinstance(op,dict) or op not in cases() or
            any(type(op[k]) is not bool for k in ('stale','postselect','cancel') if k in op)):
        raise ValueError('fixed owned face-pack command required')

def run(op,host,iterations,tolerance):
    validate(op);R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or doc.UndoRecordingIsActive or R.Commands.Command.InCommand():
        raise ValueError('empty private idle document required')
    keys=[];layers=[];groups=[];old_layer=doc.Layers.CurrentLayerIndex
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            a=obj.Attributes;g=obj.Geometry
            row=dict(source=keys.index(obj.Id) if obj.Id in keys else None,kind=g.GetType().Name,
                selected=bool(obj.IsSelected(False)),name=a.Name,layer=layers.index(a.LayerIndex),
                groups=[groups.index(i) for i in (a.GetGroupList() or [])],
                color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],crc=int(g.DataCRC(0)))
            if isinstance(g,G.SubD):
                model=R.FileIO.File3dm()
                try:model.Objects.AddSubD(g);row['archive']=[int(v) for v in model.ToByteArray()]
                finally:model.Dispose()
            rows.append(row)
        return rows
    try:
        for name in ('source','current'):layers.append(doc.Layers.Add('PackSubDFaces '+name,S.Drawing.Color.Blue))
        doc.Layers.SetCurrentLayerIndex(layers[1],True)
        for name in ('all','first','empty'):groups.append(doc.Groups.Add('PackSubDFaces '+name))
        s=make_source(source('grid' if op['scene']=='mixed' else op['scene']),host)
        try:
            if op['scene']=='subdivided_box' and not s.Subdivide(1):raise ValueError('cannot refine source')
            if op.get('stale'):
                s.PackFaces();vertices=[];vertex=s.Vertices.First
                while vertex is not None:vertices.append(vertex);vertex=vertex.Next
                indices={int(v.Id):i for i,v in enumerate(vertices)}
                for edge in s.Edges:
                    if sorted([indices[int(edge.VertexFrom.Id)],indices[int(edge.VertexTo.Id)]]) in [[2,7],[7,12],[12,17]]:
                        edge.Tag=G.SubDEdgeTag.Crease
                s.UpdateAllTagsAndSectorCoefficients();s.ClearEvaluationCache()
            nets=[s]
            if op['scene']=='mixed':
                nets += [G.Point(G.Point3d(20,0,0)),make_source(source('box'),host),G.LineCurve(G.Point3d(30,0,0),G.Point3d(31,1,2))]
            try:
                for i,net in enumerate(nets):
                    a=R.DocObjects.ObjectAttributes();a.Name='Input '+str(i);a.LayerIndex=layers[0]
                    a.ObjectColor=S.Drawing.Color.FromArgb(17+i,43+i,91+i);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
                    a.AddToGroup(groups[0])
                    if i==0:a.AddToGroup(groups[1])
                    try:keys.append(doc.Objects.Add(net,a))
                    finally:a.Dispose()
                    if keys[-1]==S.Guid.Empty:raise ValueError('cannot insert source')
            finally:
                for net in nets[1:]:net.Dispose()
        finally:s.Dispose()
        doc.Objects.UnselectAll();doc.ClearUndoRecords(True)
        if not op.get('postselect'):
            for key in keys:doc.Objects.Select(key)
        steps=[]
        for repeat in range(1 if op.get('cancel') else 2):
            before=snapshot();macro='_-PackSubDFaces'
            if op.get('postselect') or repeat:
                doc.Objects.UnselectAll()
                macro+=' '+' '.join('_SelID '+str(keys[i]) for i in reversed(range(len(keys))) if i in (0,2))
                macro+=' !' if op.get('cancel') else ' _Enter'
            marker='Owned pack '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
            success,after,events=observe_command(R.Commands.Command,'PackSubDFaces',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
            if R.Commands.Command.InCommand():raise ValueError('pack macro left command active')
            history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip();settled=snapshot()
            if op.get('cancel'):undo=redo=None
            else:
                R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
            steps.append(dict(before=before,after=after,settled=settled,undo=undo,redo=redo,history=history,success=success,
                results=[e['result'] for e in events if e['name']=='PackSubDFaces']))
        return dict(steps=steps),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        for group in reversed(groups):doc.Groups.Delete(group)
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
