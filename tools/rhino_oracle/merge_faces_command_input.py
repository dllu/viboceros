"""Bounded fixed MergeFaces input, recording a public active object getter."""


def run(host, results=None):
    if results not in (None,'Selected','NotSelected'):raise ValueError('fixed Results value required')
    import clr
    clr.AddReference('System.Core')
    from System.Linq.Expressions import Expression as E, ParameterExpression
    R,S=host['Rhino'],host['System'];doc=R.RhinoDoc.ActiveDoc
    state=S.Array[S.Int32]([1,0,0,-1,0]);prompts=S.Array[S.String]([''])
    started=S.DateTime.UtcNow;array=lambda v:S.Array[E](v)
    integer=lambda v:E.Constant(S.Int32(v))
    item=lambda i:E.ArrayAccess(E.Constant(state),array([integer(i)]))
    def method(t,n,args):return clr.GetClrType(t).GetMethod(n,S.Array[S.Type]([clr.GetClrType(a)for a in args]))
    def call(target,m,args):return E.Call(target,m,array(args))
    def block(values):return E.Block(array(values+[E.Empty()]))
    now=E.Property(None,clr.GetClrType(S.DateTime).GetProperty('UtcNow'))
    elapsed=E.Property(call(now,method(S.DateTime,'Subtract',[S.DateTime]),[E.Constant(started)]),'TotalSeconds')
    getter=call(None,method(R.Input.Custom.GetObject,'ActiveGetObject',[R.RhinoDoc]),[E.Constant(doc,clr.GetClrType(R.RhinoDoc))])
    ready=E.AndAlso(E.Equal(item(0),integer(1)),E.AndAlso(E.Equal(item(1),integer(0)),E.AndAlso(E.GreaterThan(elapsed,E.Constant(.3)),E.NotEqual(getter,E.Constant(None,clr.GetClrType(R.Input.Custom.GetObject))))))
    keys=('_Results=_'+results+' 'if results is not None else '')+'_Enter'
    send=method(R.RhinoApp,'SendKeystrokes',[S.String,S.Boolean])
    body=block([E.IfThen(ready,block([E.Assign(item(1),integer(1)),E.Assign(item(2),integer(1)),
        E.Assign(item(3),E.Property(getter,'ObjectCount')),
        E.Assign(E.ArrayAccess(E.Constant(prompts),array([integer(0)])),E.Property(None,clr.GetClrType(R.RhinoApp).GetProperty('CommandPrompt'))),
        call(None,send,[E.Constant(keys),E.Constant(True)])])),
        E.IfThen(E.AndAlso(E.Equal(item(0),integer(1)),E.GreaterThan(elapsed,E.Constant(15.))),
            block([E.Assign(item(4),integer(1)),call(None,send,[E.Constant('_Cancel'),E.Constant(True)])]))])
    action=E.Lambda(clr.GetClrType(S.Action),body).Compile()
    invoke=method(R.RhinoApp,'InvokeOnUiThread',[S.Delegate,S.Array[S.Object]])
    argument=E.Parameter(clr.GetClrType(S.Object),'state')
    callback=E.Lambda(clr.GetClrType(S.Threading.TimerCallback),block([call(None,invoke,[E.Constant(action,clr.GetClrType(S.Delegate)),E.Constant(S.Array[S.Object]([]))])]),S.Array[ParameterExpression]([argument])).Compile()
    timer=S.Threading.Timer(callback,None,100,100)
    try:returned=R.RhinoApp.RunScript('_MergeFaces',False)
    finally:state[0]=0;timer.Dispose()
    if state[4]:raise ValueError('bounded MergeFaces getter timed out')
    return dict(returned=bool(returned),active_getter_observed=bool(state[2]),object_count=int(state[3]),
                prompt=str(prompts[0]),sent=keys if state[1]else None)
