"""Owned public SubCrv MarkEnds recipes with numeric confirmation and history."""
import re
CASES=('point_forward','point_backward','forward_copy','forward_replace','backward_copy','backward_replace','negative','clamp','quadratic','closed_forward','closed_backward','closed_full','zero','no_confirmation','postselect')
def request():
    return dict(protocol_version=1,iterations=1,operations=[dict(op='subcurve_mark_ends',id='mark_ends_'+c,case=c) for c in CASES])
def validate_request(q):
    if (not isinstance(q,dict) or type(q.get('protocol_version')) is not int or q['protocol_version']!=1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1)!=1
            or not isinstance(q.get('operations'),list) or not 1<=len(q['operations'])<=32):
        raise ValueError('SubCrv MarkEnds requires bounded protocol 1 recipes')
    seen=set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op)!={'op','id','case'} or op['op']!='subcurve_mark_ends'
                or not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None
                or op['id'] in seen or op.get('case') not in CASES):raise ValueError('invalid SubCrv MarkEnds recipe')
        seen.add(op['id'])
def run(op,host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino,System=host['Rhino'],host['System'];G=Rhino.Geometry;doc=Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand():raise ValueError('SubCrv requires idle execution')
    if list(doc.Objects):raise ValueError('SubCrv requires empty owned document')
    from join_probe import observe_command
    owned=[];ids=[];layers=[];saved_tolerance=doc.ModelAbsoluteTolerance;saved_layer=doc.Layers.CurrentLayerIndex
    baseline_groups=set(i for i in range(doc.Groups.Count) if not doc.Groups.IsDeleted(i))
    doc.ModelAbsoluteTolerance=1e-6
    def keep(g):
        if g is None or not g.IsValid:raise ValueError('invalid SubCrv source')
        owned.append(g);return g
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g=obj.Geometry;a=obj.Attributes
            if isinstance(g,G.Point):
                rows.append(dict(source=ids.index(obj.Id) if obj.Id in ids else None,point=host['_xyz'](g.Location),selected=bool(obj.IsSelected(False)),name=a.Name,layer=doc.Layers[a.LayerIndex].Name,user_text=a.GetUserString('source'),groups=list(a.GetGroupList() or [])))
                continue
            n=g.ToNurbsCurve()
            try:rows.append(dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                definition=host['_nurbs_curve_definition'](n),samples=[host['_xyz'](obj.Geometry.PointAt(obj.Geometry.Domain.ParameterAt(i/32.))) for i in range(33)],
                selected=bool(obj.IsSelected(False)),name=a.Name,layer=doc.Layers[a.LayerIndex].Name,
                user_text=a.GetUserString('source'),groups=list(a.GetGroupList() or [])))
            finally:n.Dispose()
        return rows
    try:
        layer=Rhino.DocObjects.Layer();layer.Name=op['id']+'_input';layers.append(doc.Layers.Add(layer))
        layer=Rhino.DocObjects.Layer();layer.Name=op['id']+'_output';layers.append(doc.Layers.Add(layer))
        case=op['case'];copy=not case.endswith('replace')
        if 'closed' in case:curve=keep(G.PolylineCurve([G.Point3d(x,y,0.) for x,y in ((0.,0.),(4.,0.),(4.,6.),(0.,6.),(0.,0.))]))
        elif case=='quadratic':curve=keep(host['_nurbs_curve_from_definition'](dict(degree=2,knots=[2.,2.,2.,5.,5.,5.],control_points=[dict(point=p,weight=1.) for p in ((0.,0.,0.),(2.,6.,0.),(4.,6.,0.))])))
        else:curve=keep(G.LineCurve(G.Point3d(0.,0.,0.),G.Point3d(4.,6.,0.)))
        group=doc.Groups.Add('SubCrv_source_'+op['id']);a=doc.CreateDefaultAttributes();a.Name='source';a.LayerIndex=layers[0];a.SetUserString('source','original');a.AddToGroup(group)
        ids.append(doc.Objects.AddCurve(curve,a));doc.Layers.SetCurrentLayerIndex(layers[1],True)
        fraction=.8 if 'closed' in case else (.9 if case=='clamp' else (.75 if 'backward' in case else .25))
        start_parameter=curve.Domain.ParameterAt(fraction);start=host['_xyz'](curve.PointAt(start_parameter))
        confirmation=[3.,0.,0.] if case=='closed_forward' else ([4.,3.,0.] if case=='closed_backward' else ([.2,.3,0.] if 'backward' in case else [10.,10.,0.]))
        token='20' if case=='closed_full' else ('8' if 'closed' in case else ('-2' if case=='negative' else '2'))
        if case=='closed_point_forward':confirmation=[3.,0.,0.];tail='3,0,0'
        elif case=='closed_point_backward':confirmation=[4.,3.,0.];tail='4,3,0'
        elif case=='zero':tail='0'
        elif case=='no_confirmation':tail='2 _Enter'
        elif case=='point_forward':tail='3,4.5,0'
        elif case=='point_backward':tail='1,1.5,0'
        else:tail=token+' '+','.join(repr(x) for x in confirmation)
        selector='_SelID '+str(ids[0])
        if case=='postselect':macro='_SubCrv '+selector+' _Mode=_MarkEnds _Copy=_'+('Yes' if copy else 'No')+' '+','.join(repr(x) for x in start)+' '+tail+' _Enter'
        else:
            doc.Objects.Select(ids[0]);macro='_SubCrv _Mode=_MarkEnds _Copy=_'+('Yes' if copy else 'No')+' '+','.join(repr(x) for x in start)+' '+tail+' _Enter'
        doc.ClearUndoRecords(True);before=snapshot();marker='Viboceros SubCrv MarkEnds '+str(System.Guid.NewGuid());Rhino.RhinoApp.WriteLine(marker)
        success,after,events=observe_command(Rhino.Commands.Command,'SubCrv',lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        active=bool(Rhino.Commands.Command.InCommand())
        if active:Rhino.RhinoApp.RunScript('!',False)
        after_script=snapshot();history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        Rhino.RhinoApp.RunScript('_Undo',False);undo=snapshot()
        Rhino.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(copy=copy,macro=macro,start_parameter=start_parameter,start=start,confirmation=confirmation,length_token=token,
            before=before,after=after,after_script=after_script,success=success,command_active=active,events=events,history=history,undo=undo,redo=redo),0
    finally:
        if Rhino.Commands.Command.InCommand():Rhino.RhinoApp.RunScript('!',False)
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.Layers.SetCurrentLayerIndex(saved_layer,True)
        for i in range(doc.Groups.Count):
            if i not in baseline_groups and not doc.Groups.IsDeleted(i):doc.Groups.Delete(i)
        for i in reversed(layers):doc.Layers.Delete(i,True)
        doc.ModelAbsoluteTolerance=saved_tolerance
        for g in reversed(owned):g.Dispose()
