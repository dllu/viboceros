"""Supply bounded input only if HidePt opens its public grip getter."""


def run(host, pick=False, cancel=False, command='HidePt', has_points=True, selection_ids=(), accept_selection=True):
    if command not in ('HidePt','Move','PtOffSelected'):raise ValueError('unsupported grip getter')
    import clr
    R,S=host['Rhino'],host['System'];clr.AddReference('System.Core')
    from System.Linq.Expressions import Expression as E, ParameterExpression
    state=S.Array[S.Int32]([1,0,0]);started=S.DateTime.UtcNow
    array=lambda values:S.Array[E](values)
    integer=lambda value:E.Constant(S.Int32(value))
    item=lambda i:E.ArrayAccess(E.Constant(state),array([integer(i)]))
    def method(kind,name,types):return clr.GetClrType(kind).GetMethod(name,S.Array[S.Type]([clr.GetClrType(t)for t in types]))
    def call(target,member,args):return E.Call(target,member,array(args))
    def block(values):return E.Block(array(values+[E.Empty()]))
    def compile(kind,body,parameters=()):return E.Lambda(clr.GetClrType(kind),body,S.Array[ParameterExpression](parameters)).Compile()
    now=E.Property(None,clr.GetClrType(S.DateTime).GetProperty('UtcNow'))
    elapsed=E.Property(call(now,method(S.DateTime,'Subtract',[S.DateTime]),[E.Constant(started)]),'TotalSeconds')
    active=E.Equal(item(0),integer(1));prompt=E.Property(None,clr.GetClrType(R.RhinoApp).GetProperty('CommandPrompt'))
    select=call(prompt,method(S.String,'StartsWith',[S.String]),[E.Constant('Select' if command=='PtOffSelected' else ('Select control points to hide'if command=='HidePt'else'Select objects to move'))])
    ready=E.AndAlso(active,E.AndAlso(E.Equal(item(1),integer(0)),E.AndAlso(select,E.GreaterThan(elapsed,E.Constant(.3)))))
    send=method(R.RhinoApp,'SendKeystrokes',[S.String,S.Boolean])
    text=('_SelPt 'if pick else '')+('_Cancel'if cancel or not pick else '_Enter')
    if selection_ids:
        if command!='PtOffSelected' or len(selection_ids)>2:raise ValueError('invalid owned selection input')
        text=' '.join('_SelID '+str(key) for key in selection_ids)+(' _Enter' if accept_selection else ' _Cancel')
    body=block([E.IfThen(ready,block([E.Assign(item(1),integer(1)),call(None,send,[E.Constant(text),E.Constant(True)])])),
        E.IfThen(E.AndAlso(active,E.GreaterThan(elapsed,E.Constant(30.))),block([E.Assign(item(2),integer(1)),call(None,send,[E.Constant('_Cancel'),E.Constant(True)])]))])
    action=compile(S.Action,body)
    invoke=method(R.RhinoApp,'InvokeOnUiThread',[S.Delegate,S.Array[S.Object]])
    argument=E.Parameter(clr.GetClrType(S.Object),'state')
    callback=compile(S.Threading.TimerCallback,block([call(None,invoke,[E.Constant(action,clr.GetClrType(S.Delegate)),E.Constant(S.Array[S.Object]([]))])]),[argument])
    timer=S.Threading.Timer(callback,None,100,100)
    try:returned=R.RhinoApp.RunScript('_PtOffSelected'if command=='PtOffSelected'else('_HidePt'if command=='HidePt'else('_Move w0,0,0 w0,0,1'if has_points else'_Move')),False)
    finally:state[0]=0;timer.Dispose()
    if state[2]:raise ValueError('HidePt getter input timed out')
    return returned,bool(state[1]),text if state[1] else None
