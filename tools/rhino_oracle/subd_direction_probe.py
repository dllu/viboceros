"""Named Flip and Dir command edits of owned SubDs, with complete state traces."""
if __package__:
    from .subd_direction_cases import source,validate
    from .subd_to_nurbs_probe import make_source
else:
    from subd_direction_cases import source,validate
    from subd_to_nurbs_probe import make_source


def geometry(g,host):
    G=host['Rhino'].Geometry
    if isinstance(g,G.SubD):
        vertices=[];vertex=g.Vertices.First
        while vertex is not None:vertices.append(vertex);vertex=vertex.Next
        index={int(v.Id):i for i,v in enumerate(vertices)}
        return dict(kind='subd',points=[host['_xyz'](v.ControlNetPoint)for v in vertices],vertex_tags=[int(v.Tag)for v in vertices],vertex_ids=[int(v.Id)for v in vertices],edges=[[index[int(e.VertexFrom.Id)],index[int(e.VertexTo.Id)]]for e in g.Edges],edge_tags=[int(e.Tag)for e in g.Edges],edge_ids=[int(e.Id)for e in g.Edges],faces=[[index[int(f.VertexAt(i).Id)]for i in range(f.EdgeCount)]for f in g.Faces],face_ids=[int(f.Id)for f in g.Faces])
    if isinstance(g,G.Curve):return dict(kind='line',points=[host['_xyz'](g.PointAtStart),host['_xyz'](g.PointAtEnd)])
    raise ValueError('unexpected direction source')


def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    recipe=source(op['scene']);subd=make_source(recipe,host);keys=[];group=-1;old=doc.SubDAppearance
    try:
        attrs=R.DocObjects.ObjectAttributes();attrs.Name='Direction source'
        try:key=doc.Objects.AddSubD(subd,attrs)
        finally:attrs.Dispose()
        if key==S.Guid.Empty:raise ValueError('cannot add owned direction source')
        keys.append(key)
        if op['mixed']:
            attrs=R.DocObjects.ObjectAttributes();attrs.Name='Direction curve'
            try:other=doc.Objects.AddLine(G.Point3d(20,0,0),G.Point3d(23,1,0),attrs)
            finally:attrs.Dispose()
            if other==S.Guid.Empty:raise ValueError('cannot add owned direction peer')
            keys.append(other)
        group=doc.Groups.Add('Owned direction group',keys)
        if group<0:raise ValueError('cannot group direction sources')
        doc.SubDAppearance=G.SubDComponentLocation.ControlNet if op['flat']else G.SubDComponentLocation.Surface
        def snapshot():
            rows=[]
            for key in keys:
                obj=doc.Objects.FindId(key)
                rows.append(dict(geometry=geometry(obj.Geometry,host),selected=bool(obj.IsSelected(False)),name=obj.Attributes.Name,groups=int(obj.Attributes.GroupCount)))
            return dict(sources=rows,objects=len(list(doc.Objects)),points=[host['_xyz'](obj.Geometry.Location)for obj in doc.Objects if isinstance(obj.Geometry,G.Point)])
        doc.Objects.UnselectAll();doc.ClearUndoRecords(True)
        R.RhinoApp.RunScript('_Point w40,50,60',False);R.RhinoApp.RunScript('_Undo',False)
        if op['preselect']:
            for key in keys:doc.Objects.Select(key)
        before=snapshot();marker='Owned SubD direction '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        select=''if op['preselect']else' '.join('_SelID '+str(k)for k in keys)+' _Enter '
        macro='_'+op['command']+' '+select+(' '.join('_'+step for step in op['steps'])+' 'if op['command']=='Dir'else'')+(' !'if op['cancel']else(''if op.get('bare')else' _Enter'))
        success,event_state,events=observe_command(R.Commands.Command,op['command'],lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],trace=True)
        if R.Commands.Command.InCommand():raise ValueError('direction command left an active getter')
        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        changed=any(a['geometry']!=b['geometry']for a,b in zip(before['sources'],after['sources']))
        if changed:
            R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        else:
            R.RhinoApp.RunScript('_Redo',False);redo=snapshot();R.RhinoApp.RunScript('_Undo',False);undo=snapshot()
        return dict(input=recipe,success=success,before=before,event_state=event_state,after=after,undo=undo,redo=redo,history=history,history_after=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip(),macro=macro,events=events,changed=changed),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        if group>=0:doc.Groups.Delete(group)
        doc.SubDAppearance=old;subd.Dispose()
