"""Owned SelEdgeLoop selection lifecycle; no source geometry is edited."""
if __package__:
    from .edge_loop_probe import Input
else:
    from edge_loop_probe import Input

def discovery_cases():
    rows=[]
    for kind in ('mesh','subd'):
        for name,initial,steps,finish in [
            ('empty_enter',[],[],'Enter'),
            ('pre_enter',[[6,7]],[],'Enter'),
            ('picked_accept',[],[dict(kind='click',modifiers='plain',pair=[6,7])],'Enter'),
            ('picked_cancel',[],[dict(kind='click',modifiers='plain',pair=[6,7])],'Cancel')]:
            rows.append(dict(op='edge_loop_workflow',id=kind+'_'+name,kind=kind,scene='grid',pair=[6,7],initial=initial,steps=steps,finish=finish))
    return rows

def validate(op):
    if not isinstance(op,dict) or op not in discovery_cases()+cases() or ('whole' in op and type(op['whole']) is not bool) or any(type(i) is not int for pair in [op.get('pair',[])]+op.get('initial',[])+[s.get('pair',[]) for s in op.get('steps',[])] for i in pair):
        raise ValueError('fixed owned edge-loop workflow required')

def cases():
    h=dict(kind='click',modifiers='plain',pair=[6,7]);v=dict(kind='click',modifiers='plain',pair=[6,11]);same=dict(kind='click',modifiers='plain',pair=[7,8])
    remove=dict(h,modifiers='ctrl')
    rows=[]
    for kind in ('mesh','subd'):
        for name,initial,steps,finish,flags in [
            ('whole_accept',[],[h],'Enter',dict(whole=True)),('whole_cancel',[],[h],'Cancel',dict(whole=True)),
            ('two_loops',[],[h,v],'Enter',{}),('two_same',[],[h,same],'Enter',{}),
            ('remove_seed',[],[h,remove],'Enter',{}),('remove_nonseed',[],[h,dict(same,modifiers='ctrl')],'Enter',{}),
            ('remove_cross',[],[h,v,remove],'Enter',{}),('remove_one_same',[],[h,same,remove],'Enter',{}),
            ('cross_cancel',[],[h,v],'Cancel',{}),('pre_cross',[[6,7],[6,11]],[],'Enter',{}),
            ('pre_same',[[6,7],[7,8]],[],'Enter',{}),('pre_face',[],[],'Enter',dict(component='face')),
            ('face_picked',[],[h],'Enter',dict(component='face')),('pre_vertex',[],[],'Enter',dict(component='vertex')),
            ('vertex_picked',[],[h],'Enter',dict(component='vertex')),('remove_readd',[],[h,remove,h],'Enter',{}),
            ('repeat_seed',[],[h,h],'Enter',{}),('ctrl_unselected',[],[remove],'Enter',{})]:
            rows.append(dict(dict(op='edge_loop_workflow',id=kind+'_'+name,kind=kind,scene='grid',pair=[6,7],initial=initial,steps=steps,finish=finish),**flags))
    return rows

def run(op,host,iterations,tolerance):
    validate(op)
    return run_owned(op,host,iterations,tolerance,'SelEdgeLoop')

def run_owned(op,host,iterations,tolerance,command,input_factory=Input):
    R,S=host['Rhino'],host['System'];doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('empty private idle document required')
    R.RhinoApp.RunScript('_SelectionFilterNone',False)
    with input_factory(op,host) as value:
        if not R.RhinoApp.RunScript('_Point w40,50,60',False):raise ValueError('cannot seed owned redo')
        if not R.RhinoApp.RunScript('_Undo',False) or len(list(doc.Objects))!=1:raise ValueError('cannot prepare owned redo')
        obj=doc.Objects.FindId(value.key)
        if op.get('whole'):doc.Objects.Select(value.key)
        if op.get('component'):
            face=op['component']=='face'
            kind=(R.Geometry.ComponentIndexType.MeshFace if face else R.Geometry.ComponentIndexType.MeshTopologyVertex) if op['kind']=='mesh' else (R.Geometry.ComponentIndexType.SubdFace if face else R.Geometry.ComponentIndexType.SubdVertex)
            index=0 if op['kind']=='mesh' else int((list(obj.Geometry.Faces)[0] if face else obj.Geometry.Vertices.First).Id)
            if not obj.SelectSubObject(R.Geometry.ComponentIndex(kind,index),True,True,False):raise ValueError('cannot seed owned nonedge input')
        for pair in op['initial']:
            index=next(i for i,e in enumerate(value.edges) if set(e)==set(pair))
            kind=R.Geometry.ComponentIndexType.MeshTopologyEdge if op['kind']=='mesh' else R.Geometry.ComponentIndexType.SubdEdge
            if not obj.SelectSubObject(R.Geometry.ComponentIndex(kind,value.native[index]),True,True,False):raise ValueError('cannot seed owned component input')
        def snapshot():
            obj=doc.Objects.FindId(value.key)
            components=[dict(kind=str(c.ComponentIndexType),index=int(c.Index)) for c in (obj.GetSelectedSubObjects() or [])]
            edge_kind='MeshTopologyEdge' if op['kind']=='mesh' else 'SubdEdge'
            return dict(whole=bool(obj.IsSelected(False)),components=components,ordinals=[value.native.index(c['index']) for c in components if c['kind']==edge_kind],objects=len(list(doc.Objects)))
        before=snapshot();trace=[];events=[]
        marker='Owned loop workflow '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        def ended(sender,event):
            if event.CommandEnglishName==command:events.append(dict(result=str(event.CommandResult),selection=snapshot()))
        R.Commands.Command.EndCommand+=ended
        try:
            if op['steps']:
                from shrink_face_input import drive
                points=[]
                for step in op['steps']:
                    a,b=[R.Geometry.Point3d(*value.recipe['points'][i]) for i in step['pair']]
                    points.append([R.Geometry.Point3d((a.X+b.X)*.5,(a.Y+b.Y)*.5,(a.Z+b.Z)*.5)])
                success=drive(op,points,host,snapshot,trace,command)
            else:success=R.RhinoApp.RunScript('_'+command+' _'+('Enter' if op['finish']=='Enter' else 'Cancel'),True)
        finally:R.Commands.Command.EndCommand-=ended
        if R.Commands.Command.InCommand():raise ValueError('owned loop workflow left active input')
        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot()
        return dict(input=value.recipe,archive=value.archive,edges=value.edges,native=value.native,
                    before=before,trace=trace,events=events,after=after,redo=redo,undo=undo,history=history,success=success,
                    source_unchanged=int(doc.Objects.FindId(value.key).Geometry.DataCRC(0))==value.crc),0
