"""Managed delegates deliver bounded physical picks while Python awaits RunScript."""
import json
import os


def drive(host, script, identifier, views, select_grips=False, command='ExtractPt'):
    if command not in ('ExtractPt','MergeFaces') or command=='MergeFaces' and select_grips:
        raise ValueError('fixed owned component command required')
    import clr
    clr.AddReference('System.Core')
    from System.Linq.Expressions import Expression as E, ParameterExpression
    R,S=host['Rhino'],host['System']
    doc=R.RhinoDoc.ActiveDoc;view=doc.Views.ActiveView;vp=view.ActiveViewport
    root=os.path.dirname(host['__file__']);progress=os.path.join(root,'worker-progress.log');ack=os.path.join(root,'click-ack.json')
    state=S.Array[S.Int32]([0,1,0,0,0,0]);sent=S.Array[S.Int32]([0]*len(views));counts=S.Array[S.Int32]([-1]*len(views));flags=S.Array[S.Int32]([0]*len(views))
    last=S.Array[S.DateTime]([S.DateTime.UtcNow]);started=S.DateTime.UtcNow
    array=lambda values:S.Array[E](values)
    integer=lambda value:E.Constant(S.Int32(value))
    item=lambda values,index:E.ArrayAccess(E.Constant(values),array([integer(index)]))
    def method(kind,name,types):return clr.GetClrType(kind).GetMethod(name,S.Array[S.Type]([clr.GetClrType(t)for t in types]))
    def call(target,member,args):return E.Call(target,member,array(args))
    def block(values):return E.Block(array(values+[E.Empty()]))
    def delegate(kind,body,parameters=()):return E.Lambda(clr.GetClrType(kind),body,S.Array[ParameterExpression](parameters)).Compile()
    now=E.Property(None,clr.GetClrType(S.DateTime).GetProperty('UtcNow'))
    elapsed=lambda stamp:E.Property(call(now,method(S.DateTime,'Subtract',[S.DateTime]),[stamp]),'TotalSeconds')
    active=E.Equal(item(state,1),integer(1));stage=item(state,0)
    prompt=E.Property(None,clr.GetClrType(R.RhinoApp).GetProperty('CommandPrompt'))
    gate=call(prompt,method(S.String,'StartsWith',[S.String]),[E.Constant('Select objects for point extraction' if command=='ExtractPt' else 'Select mesh and SubD')])
    def log(message):return call(None,method(S.IO.File,'AppendAllText',[S.String,S.String]),[E.Constant(progress),E.Constant(message)])
    append=method(S.IO.File,'AppendAllText',[S.String,S.String]);read=method(S.IO.File,'ReadAllText',[S.String]);exists=method(S.IO.File,'Exists',[S.String])
    getter=call(None,method(R.Input.Custom.GetObject,'ActiveGetObject',[R.RhinoDoc]),[E.Constant(doc,clr.GetClrType(R.RhinoDoc))])
    branches=[];markers=[]
    reference=method(R.Input.Custom.GetObject,'Object',[S.Int32])
    for i,(screen,obj,aliases) in enumerate(views):
        marker='@component-click:%s:%d:sub'%(identifier,i);markers.append(marker)
        first=isinstance(obj.Geometry,R.Geometry.SubD) or (isinstance(obj.Geometry,R.Geometry.Brep) and aliases[0].ComponentIndexType==R.Geometry.ComponentIndexType.BrepFace)
        menu='@component-menu:%s-%d:%s'%(identifier,i,'First' if first else 'Second')
        emit=block([E.Assign(item(sent,i),integer(1)),E.Assign(item(last,0),now),log('\nPICK %s %d %d\n'%(marker,screen.X,screen.Y))])
        expected=E.Condition(E.Equal(item(sent,i),integer(1)),E.Constant(json.dumps(marker)),E.Constant(json.dumps(menu)))
        acknowledged=E.AndAlso(call(None,exists,[E.Constant(ack)]),E.Equal(call(None,read,[E.Constant(ack)]),expected))
        receipt=E.Condition(E.NotEqual(getter,E.Constant(None,clr.GetClrType(R.Input.Custom.GetObject))),E.Property(getter,'ObjectCount'),integer(-1))
        selected=E.Constant(False);promoted=E.Constant(False)
        for j in reversed(range(32)):
            ref=call(getter,reference,[integer(j)])
            owner=E.Equal(E.Property(ref,'ObjectId'),E.Constant(obj.Id,clr.GetClrType(S.Guid)))
            ci=E.Property(ref,'GeometryComponentIndex')
            matches=E.Constant(False)
            for component in aliases:
                same=E.AndAlso(E.Equal(E.Property(ci,'ComponentIndexType'),E.Constant(component.ComponentIndexType,clr.GetClrType(R.Geometry.ComponentIndexType))),E.Equal(E.Property(ci,'Index'),integer(component.Index)))
                matches=E.OrElse(matches,same)
            found=E.AndAlso(E.LessThan(integer(j),E.Property(getter,'ObjectCount')),E.AndAlso(owner,matches))
            selected=E.OrElse(found,selected)
            if any(c.ComponentIndexType==R.Geometry.ComponentIndexType.InvalidType for c in aliases):
                whole=E.AndAlso(E.Equal(E.Property(ci,'ComponentIndexType'),E.Constant(R.Geometry.ComponentIndexType.InvalidType,clr.GetClrType(R.Geometry.ComponentIndexType))),E.Equal(E.Property(ci,'Index'),integer(-1)))
                promoted=E.OrElse(E.AndAlso(found,whole),promoted)
        selected=E.AndAlso(E.NotEqual(getter,E.Constant(None,clr.GetClrType(R.Input.Custom.GetObject))),selected)
        admitted=block([E.Assign(item(flags,i),E.Condition(promoted,integer(2),integer(1))),E.Assign(item(counts,i),receipt),log('ACK %d expected reference admitted\n'%i),E.Assign(stage,integer(i+1)),E.Assign(item(last,0),now)])
        request_menu=block([E.Assign(item(sent,i),integer(2)),E.Assign(item(last,0),now),log('\nPICK %s 1 1\n'%menu)])
        diagnostics=[E.Assign(item(counts,i),receipt)]
        fmt=method(S.String,'Format',[S.String,S.Object,S.Object,S.Object])
        for j in range(4):
            ref=call(getter,reference,[integer(j)]);ci=E.Property(ref,'GeometryComponentIndex')
            line=call(None,fmt,[E.Constant('Getter ref: {0} {1} {2}\n'),E.Convert(E.Property(ref,'ObjectId'),clr.GetClrType(S.Object)),E.Convert(E.Property(ci,'ComponentIndexType'),clr.GetClrType(S.Object)),E.Convert(E.Property(ci,'Index'),clr.GetClrType(S.Object))])
            diagnostics.append(E.IfThen(E.AndAlso(E.NotEqual(getter,E.Constant(None,clr.GetClrType(R.Input.Custom.GetObject))),E.LessThan(integer(j),E.Property(getter,'ObjectCount'))),call(None,append,[E.Constant(progress),line])))
        reject=block(diagnostics+[E.Assign(item(state,2),integer(1)),log('\nPICK_ABORT %s\n'%identifier)])
        resolved=E.IfThenElse(selected,admitted,E.IfThenElse(E.Equal(item(sent,i),integer(1)),request_menu,E.IfThen(E.GreaterThan(elapsed(item(last,0)),E.Constant(5.)),reject)))
        advance=E.IfThen(E.AndAlso(acknowledged,E.GreaterThan(elapsed(item(last,0)),E.Constant(.6))),resolved)
        branches.append(E.IfThen(E.Equal(stage,integer(i)),E.IfThenElse(E.Equal(item(sent,i),integer(0)),emit,advance)))
    finish_marker='@component-finish:%s:Enter'%identifier
    if select_grips:
        key='@component-key:%s:99:SelPt'%identifier
        key_ack=E.AndAlso(call(None,exists,[E.Constant(ack)]),E.Equal(call(None,read,[E.Constant(ack)]),E.Constant(json.dumps(key))))
        after_key=E.IfThen(E.AndAlso(key_ack,E.GreaterThan(elapsed(item(last,0)),E.Constant(.6))),block([E.Assign(item(state,3),integer(2)),log('\nPICK %s 1 1\n'%finish_marker)]))
        finish_body=E.IfThenElse(E.Equal(item(state,3),integer(0)),block([E.Assign(item(state,3),integer(1)),E.Assign(item(last,0),now),log('\nPICK %s 1 1\n'%key)]),E.IfThen(E.Equal(item(state,3),integer(1)),after_key))
    else:finish_body=E.IfThen(E.Equal(item(state,3),integer(0)),block([E.Assign(item(state,3),integer(2)),log('\nPICK %s 1 1\n'%finish_marker)]))
    finish=E.IfThen(E.Equal(stage,integer(len(views))),finish_body)
    done=E.Label();body=[]
    for i,branch in enumerate(branches):body.append(E.IfThen(E.Equal(stage,integer(i)),block([branch,E.Goto(done)])))
    body.extend([finish,E.Label(done)])
    action=delegate(S.Action,E.IfThen(E.AndAlso(active,E.AndAlso(E.Equal(item(state,2),integer(0)),E.AndAlso(gate,E.GreaterThan(elapsed(item(last,0)),E.Constant(.4))))),block(body)))
    cancel=delegate(S.Action,E.IfThen(E.AndAlso(active,E.AndAlso(E.Equal(item(state,2),integer(0)),E.GreaterThan(elapsed(E.Constant(started)),E.Constant(45.)))),block([E.Assign(item(state,2),integer(1)),log('\nPICK_ABORT %s\n'%identifier)])))
    invoke=method(R.RhinoApp,'InvokeOnUiThread',[S.Delegate,S.Array[S.Object]])
    def callback(action):
        argument=E.Parameter(clr.GetClrType(S.Object),'state')
        return delegate(S.Threading.TimerCallback,block([call(None,invoke,[E.Constant(action,clr.GetClrType(S.Delegate)),E.Constant(S.Array[S.Object]([]))])]),[argument])
    event=clr.GetClrType(R.Commands.Command).GetEvent('EndCommand')
    parameters=event.EventHandlerType.GetMethod('Invoke').GetParameters()
    sender=E.Parameter(parameters[0].ParameterType,'sender');ended=E.Parameter(parameters[1].ParameterType,'event')
    is_command=E.Equal(E.Property(ended,'CommandEnglishName'),E.Constant(command))
    succeeded=E.Equal(E.Property(ended,'CommandResult'),E.Constant(R.Commands.Result.Success,clr.GetClrType(R.Commands.Result)))
    body=E.IfThen(is_command,block([E.Assign(item(state,4),E.Add(item(state,4),integer(1))),E.Assign(item(state,5),E.Condition(succeeded,integer(1),integer(2))),log(command+' managed EndCommand\n')]))
    handler=E.Lambda(event.EventHandlerType,body,S.Array[ParameterExpression]([sender,ended])).Compile()
    event.AddEventHandler(None,handler)
    timer=S.Threading.Timer(callback(action),None,100,100);timeout=S.Threading.Timer(callback(cancel),None,500,500)
    try:returned=R.RhinoApp.RunScript(script,False)
    finally:
        state[1]=0;timer.Dispose();timeout.Dispose();event.RemoveEventHandler(None,handler)
    if state[4]!=1 or state[2] or not state[3] or state[0]!=len(views) or any(not flag for flag in flags):raise ValueError('incomplete owned ExtractPt picks: stage=%s counts=%s timeout=%s; history=%s'%(state[0],list(counts),state[2],R.RhinoApp.CommandHistoryWindowText[-1400:]+'; trace='+S.IO.File.ReadAllText(progress)[-2200:]))
    return returned,dict(selected=True,entered=True,command_count=int(state[4]),command_success=(state[5]==1),input='owned physical Ctrl+Shift clicks acknowledged by the host',getter_counts=list(counts),component_admitted=[flag==1 for flag in flags],owner_promoted=[flag==2 for flag in flags],menus=[sent[i]==2 for i in range(len(views))],markers=markers,script=script)
