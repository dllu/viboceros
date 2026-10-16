"""Owned CLR expression delegates drive public SDK getters without Python callbacks."""

def drive(op,host,parent_id,edge_index,point):
    R,S=host['Rhino'],host['System']
    if R.Commands.Command.InCommand():raise ValueError('owned edge input requires idle execution')
    import os
    import clr
    clr.AddReference('System.Core')
    from System.Linq.Expressions import Expression as E, ParameterExpression
    doc=R.RhinoDoc.ActiveDoc;obj=doc.Objects.FindId(parent_id)
    host['_record_progress']('edge-guide: building owned managed delegates')
    def array(values):return S.Array[E](values)
    def integer(value):return E.Constant(S.Int32(value))
    def item(values,index):return E.ArrayAccess(values,array([integer(index)]))
    def call(target,method,args):return E.Call(target,method,array(args))
    def block(values):return E.Block(array(values+[E.Empty()]))
    def method(t,name,types):return clr.GetClrType(t).GetMethod(name,S.Array[S.Type]([clr.GetClrType(v)for v in types]))
    def compile_delegate(t,body,parameters=()):return E.Lambda(clr.GetClrType(t),body,S.Array[ParameterExpression](parameters)).Compile()
    state=S.Array[S.Int32]([0,1,0]);indices=E.Constant(state)
    stage=item(indices,0);active=E.Equal(item(indices,1),integer(1))
    prompt=E.Property(None,clr.GetClrType(R.RhinoApp).GetProperty('CommandPrompt'))
    starts=method(S.String,'StartsWith',[S.String])
    contains=method(S.String,'Contains',[S.String])
    send=method(R.RhinoApp,'SendKeystrokes',[S.String,S.Boolean])
    def send_token(token):return call(None,send,[E.Constant(token),E.Constant(True)])
    sub=op.get('subcurve');on_curve=op['options'].get('on_curve')
    steps=[('Base point','_OnCurve'if on_curve else point(op['base']))]
    if sub:steps.extend([('Select','_SubCrv'),('Start of curve',point(sub['start'])),('curve length',point(sub['end']))])
    if on_curve:steps.append(('Base point',point(op['base'])))
    for key,value in sorted(op['options'].items()):
        if key=='copy':steps.append(('New base point','_Copy=_'+('Yes'if value else'No')))
        elif key=='rotate':steps.append(('New base point','_Rotate=_Yes'))
        elif key!='on_curve'and value:steps.append(('New base point','_'+key))
    steps.append(('New base point',point(op['target'])))
    if 'rotate'in op['options']:steps.append(('Angle',repr(float(op['options']['rotate']))))
    if op['options'].get('copy'):steps.append(('New base point','_Enter'))
    progress=os.path.join(os.path.dirname(host['__file__']),'worker-progress.log')
    heartbeat=os.path.join(os.path.dirname(host['__file__']),'edge-input-prompt.txt')
    write=method(S.IO.File,'WriteAllText',[S.String,S.String]);append=method(S.IO.File,'AppendAllText',[S.String,S.String])
    view=doc.Views.ActiveView;vp=view.ActiveViewport
    vp.SetProjection(R.Display.DefinedViewportProjection.Top,'Owned edge input',False)
    vp.SetCameraDirection(R.Geometry.Vector3d(-1.,1.,-1.),True);vp.CameraUp=R.Geometry.Vector3d(0.,0.,1.)
    bounds=obj.Geometry.GetBoundingBox(True);bounds.Inflate(1.);vp.ZoomBoundingBox(bounds)
    parent=obj.Geometry.ToBrep()if isinstance(obj.Geometry,R.Geometry.Surface)else obj.Geometry
    edge=parent.Edges[edge_index];aim=edge.PointAt(edge.Domain.ParameterAt(.375))
    pixel=vp.WorldToClient(aim);screen=view.ClientToScreen(S.Drawing.Point(int(pixel.X),int(pixel.Y)))
    if parent is not obj.Geometry and isinstance(obj.Geometry,R.Geometry.Surface):parent.Dispose()
    doc.Views.Redraw()
    gate=E.AndAlso(E.Equal(stage,integer(2 if sub else 1)),E.AndAlso(call(prompt,starts,[E.Constant('Select curve to temporarily shorten'if sub else'Select orientation curve')]),E.Equal(item(indices,2),integer(0))))
    marker='\nPICK @orient-edge:%s %d %d\n'%(op['id'],screen.X,screen.Y)
    request_click=E.IfThen(gate,block([E.Assign(item(indices,2),integer(1)),call(None,append,[E.Constant(progress),E.Constant(marker)])]))
    observed=S.Array[S.String]([None]*len(steps));seen=E.Constant(observed)
    branches=[]
    for i,(prefix,token)in enumerate(steps):
        condition=call(prompt,contains if prefix in('curve length','angle')else starts,[E.Constant(prefix)])
        actions=[E.Assign(item(seen,i),prompt)]
        actions.extend([E.Assign(stage,integer(i+1)),send_token(token)])
        branches.append(E.AndAlso(E.Equal(stage,integer(i)),condition))
        branches[-1]=E.IfThen(branches[-1],block(actions))
    # Only one stage advances on each invocation, even while SendKeystrokes queues input.
    done=E.Label()
    actions=[call(None,write,[E.Constant(heartbeat),prompt])]
    actions.append(request_click)
    for i,branch in enumerate(branches):
        actions.append(E.IfThen(E.Equal(stage,integer(i)),block([branch,E.Goto(done)])))
    actions.append(E.Label(done))
    action=compile_delegate(S.Action,E.IfThen(active,block(actions)))
    cancel=compile_delegate(S.Action,E.IfThen(active,block([send_token('_Cancel')])))
    invoke=method(R.RhinoApp,'InvokeOnUiThread',[S.Delegate,S.Array[S.Object]])
    def callback(action):
        argument=E.Parameter(clr.GetClrType(S.Object),'state')
        body=call(None,invoke,[E.Constant(action,clr.GetClrType(S.Delegate)),E.Constant(S.Array[S.Object]([]))])
        return compile_delegate(S.Threading.TimerCallback,block([body]),[argument])
    host['_record_progress']('edge-guide: compiled delegate command starting')
    timer=S.Threading.Timer(callback(action),None,100,100)
    timeout=S.Threading.Timer(callback(cancel),None,20000,500)
    try:returned=R.RhinoApp.RunScript('_OrientOnCrv _Pause',True)
    finally:state[1]=0;timer.Dispose();timeout.Dispose()
    return bool(returned),dict(prompts=list(observed),sent=[s[1]for s in steps[:state[0]]],completed=int(state[0]),expected=len(steps),click_requested=bool(state[2]),errors=[]if state[0]==len(steps)else['incomplete owned input'])
