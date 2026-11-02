"""Managed delegates answer only the owned getter while WPF calls native input."""


def start(R,S,op,target):
    import clr
    import os,json
    clr.AddReference('System.Core')
    from System.Linq.Expressions import Expression as E, ParameterExpression
    A=lambda values:S.Array[E](values)
    integer=lambda value:E.Constant(S.Int32(value))
    def method(kind,name,types):return clr.GetClrType(kind).GetMethod(name,S.Array[S.Type]([clr.GetClrType(t) for t in types]))
    def call(target,member,args):return E.Call(target,member,A(args))
    def block(values):return E.Block(A(values+[E.Empty()]))
    def delegate(kind,body,args=()):return E.Lambda(clr.GetClrType(kind),body,S.Array[ParameterExpression](args)).Compile()
    state=S.Array[S.Int32]([0,1,0]);prompts=S.Array[S.String](['']*3)
    item=lambda values,index:E.ArrayAccess(E.Constant(values),A([integer(index)]))
    now=E.Property(None,clr.GetClrType(S.DateTime).GetProperty('UtcNow'))
    last=S.Array[S.DateTime]([S.DateTime.UtcNow]);started=S.DateTime.UtcNow
    elapsed=lambda stamp:E.Property(call(now,method(S.DateTime,'Subtract',[S.DateTime]),[stamp]),'TotalSeconds')
    prompt=E.Property(None,clr.GetClrType(R.RhinoApp).GetProperty('CommandPrompt'))
    lower=call(prompt,method(S.String,'ToLowerInvariant',[]),[])
    contains=lambda text:call(lower,method(S.String,'Contains',[S.String]),[E.Constant(text)])
    send=method(R.RhinoApp,'SendKeystrokes',[S.String,S.Boolean])
    tokens=[];conditions=[];edge_pick=None
    if op['mode']=='TwoPoints':
        tokens=['w'+','.join(str(x) for x in p) for p in op['points']]
        if op['cancel']:tokens[1]='_Cancel'
        conditions=[contains('first point for direction line'),contains('second point for direction line')]
    else:
        if op['curve']=='BoxEdge' and not op['cancel']:
            doc=R.RhinoDoc.ActiveDoc;view=doc.Views.ActiveView
            edge=doc.Objects.FindId(target).Geometry.Edges[op['edge']]
            at=edge.PointAtNormalizedLength(.4)
            pixel=view.ActiveViewport.WorldToClient(at)
            screen=view.ClientToScreen(S.Drawing.Point(int(pixel.X),int(pixel.Y)))
            marker='@draft-pull-edge:'+op['id']
            tokens=[marker];conditions=[contains('select')]
            edge_pick=(marker,screen)
        elif op['cancel']:
            tokens=['_Cancel'];conditions=[contains('select')]
        else:
            tokens=['_SelID',str(target),'']
            getter=call(None,method(R.Input.Custom.GetObject,'ActiveGetObject',[R.RhinoDoc]),[E.Constant(R.RhinoDoc.ActiveDoc,clr.GetClrType(R.RhinoDoc))])
            selected=E.AndAlso(E.NotEqual(getter,E.Constant(None,clr.GetClrType(R.Input.Custom.GetObject))),E.GreaterThan(E.Property(getter,'ObjectCount'),integer(0)))
            conditions=[E.AndAlso(contains('select'),E.OrElse(contains('curve'),contains('edge'))),contains('object id to select'),E.AndAlso(contains('select'),selected)]
    stage=item(state,0);active=E.Equal(item(state,1),integer(1))
    branches=[]
    for i,(token,condition) in enumerate(zip(tokens,conditions)):
        gate=E.AndAlso(E.Equal(stage,integer(i)),condition)
        actions=[E.Assign(item(prompts,i),prompt),E.Assign(stage,integer(i+1)),E.Assign(item(last,0),now)]
        if edge_pick is not None:
            marker,screen=edge_pick
            path=os.path.join(os.path.dirname(__file__),'worker-progress.log')
            append=method(S.IO.File,'AppendAllText',[S.String,S.String])
            actions.append(call(None,append,[E.Constant(path),E.Constant('\nPICK %s %d %d\n'%(marker,screen.X,screen.Y))]))
        else:actions.append(call(None,send,[E.Constant(token),E.Constant(True)]))
        emit=block(actions)
        branches.append((gate,emit))
    chain=E.Empty()
    for gate,emit in reversed(branches):chain=E.IfThenElse(gate,emit,chain)
    action=delegate(S.Action,E.IfThen(E.AndAlso(active,E.GreaterThan(elapsed(item(last,0)),E.Constant(.25))),chain))
    timeout=delegate(S.Action,E.IfThen(E.AndAlso(active,E.GreaterThan(elapsed(E.Constant(started)),E.Constant(30.))),block([E.Assign(item(state,2),integer(1)),E.Assign(item(state,1),integer(0)),call(None,send,[E.Constant('_Cancel'),E.Constant(True)])])))
    invoke=method(R.RhinoApp,'InvokeOnUiThread',[S.Delegate,S.Array[S.Object]])
    def callback(action):
        argument=E.Parameter(clr.GetClrType(S.Object),'state')
        return delegate(S.Threading.TimerCallback,block([call(None,invoke,[E.Constant(action,clr.GetClrType(S.Delegate)),E.Constant(S.Array[S.Object]([]))])]),[argument])
    timer=S.Threading.Timer(callback(action),None,100,100)
    guard=S.Threading.Timer(callback(timeout),None,500,500)
    closed=[False]
    def stop():
        state[1]=0
        if not closed[0]:
            closed[0]=True;timer.Dispose();guard.Dispose()

        acknowledged=False
        if edge_pick is not None and state[0]>0:
            path=os.path.join(os.path.dirname(__file__),'click-ack.json')
            for unused in range(300):
                if S.IO.File.Exists(path) and S.IO.File.ReadAllText(path)==json.dumps(edge_pick[0]):acknowledged=True;break
                S.Threading.Thread.CurrentThread.Join(10)
        result=dict(edge_pick_acknowledged=acknowledged,sent=[dict(token=tokens[i],prompt=str(prompts[i])) for i in range(state[0])],errors=[])
        if state[2]:raise ValueError('owned draft pull getter timed out: '+str(result)+'; prompt='+str(R.RhinoApp.CommandPrompt))
        return result
    return stop
