"""Physical boundary clicks and Escape in a bounded owned 3DFace draft."""
import json,os


def drive(op,case,command,host):
 R,S=host['Rhino'],host['System'];doc=R.RhinoDoc.ActiveDoc
 import clr
 clr.AddReference('System.Core')
 from System.Linq.Expressions import Expression as E,ParameterExpression
 picks=[p['edge_pick']for p in case['points']if isinstance(p,dict)]
 view=doc.Views.ActiveView;vp=view.ActiveViewport
 vp.SetProjection(R.Display.DefinedViewportProjection.Top,'Owned 3DFace chain',False)
 vp.ZoomBoundingBox(R.Geometry.BoundingBox(R.Geometry.Point3d(-5.,-4.,-2.),R.Geometry.Point3d(12.,8.,5.)));view.Redraw()
 markers=[];names=[]
 for i,p in enumerate(picks):
  pixel=vp.WorldToClient(R.Geometry.Point3d(*p));screen=view.ClientToScreen(S.Drawing.Point(int(pixel.X),int(pixel.Y)))
  name=op['id']+'-'+str(i);names.append(name);markers.append('\nPICK %s %d %d\n'%(name,screen.X,screen.Y))
 if case.get('cancel'):
  for i in range(4):names.append(None);markers.append('\nPICK_ABORT %s-cancel-%d\n'%(op['id'],i))
 state=S.Array[S.Int32]([0,1]);observed=S.Array[S.String]([None]*len(markers))
 root=os.path.dirname(host['__file__']);progress=os.path.join(root,'worker-progress.log');ack=os.path.join(root,'click-ack.json')
 def array(v):return S.Array[E](v)
 def integer(v):return E.Constant(S.Int32(v))
 def item(v,i):return E.ArrayAccess(E.Constant(v),array([integer(i)]))
 def method(t,name,types):return clr.GetClrType(t).GetMethod(name,S.Array[S.Type]([clr.GetClrType(v)for v in types]))
 def call(target,m,args):return E.Call(target,m,array(args))
 def block(v):return E.Block(array(v+[E.Empty()]))
 def compile_delegate(t,body,parameters=()):return E.Lambda(clr.GetClrType(t),body,S.Array[ParameterExpression](parameters)).Compile()
 prompt=E.Property(None,clr.GetClrType(R.RhinoApp).GetProperty('CommandPrompt'))
 starts=method(S.String,'StartsWith',[S.String]);append=method(S.IO.File,'AppendAllText',[S.String,S.String])
 exists=method(S.IO.File,'Exists',[S.String]);read=method(S.IO.File,'ReadAllText',[S.String])
 in_point=method(R.Input.RhinoGet,'InGetPoint',[R.RhinoDoc]);stage=item(state,0)
 gate=E.AndAlso(call(prompt,starts,[E.Constant('Pick point')]),call(None,in_point,[E.Constant(doc,clr.GetClrType(R.RhinoDoc))]))
 body=E.Empty()
 for i in reversed(range(len(markers))):
  ready=gate
  if i and names[i-1]is not None:
   acknowledged=E.AndAlso(call(None,exists,[E.Constant(ack)]),E.Equal(call(None,read,[E.Constant(ack)]),E.Constant(json.dumps(names[i-1]))))
   ready=E.AndAlso(ready,acknowledged)
  action=E.IfThen(ready,block([E.Assign(item(observed,i),prompt),E.Assign(stage,integer(i+1)),call(None,append,[E.Constant(progress),E.Constant(markers[i])])]))
  body=E.IfThenElse(E.Equal(stage,integer(i)),action,body)
 action=compile_delegate(S.Action,E.IfThen(E.Equal(item(state,1),integer(1)),body))
 invoke=method(R.RhinoApp,'InvokeOnUiThread',[S.Delegate,S.Array[S.Object]]);parameter=E.Parameter(clr.GetClrType(S.Object),'state')
 callback=compile_delegate(S.Threading.TimerCallback,block([call(None,invoke,[E.Constant(action,clr.GetClrType(S.Delegate)),E.Constant(S.Array[S.Object]([]))])]),[parameter])
 timer=S.Threading.Timer(callback,None,500,1500)
 try:returned=R.RhinoApp.RunScript(command,True)
 finally:state[1]=0;timer.Dispose()
 if state[0]<len(picks):raise ValueError('incomplete owned 3DFace boundary input')
 return bool(returned),dict(requests=int(state[0]),edge_requests=len(picks),prompts=list(observed)[:state[0]],input='owned physical boundary clicks and Escape'if case.get('cancel')else'owned physical boundary clicks')
