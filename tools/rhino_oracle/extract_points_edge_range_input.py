"""One bounded physical edge pick inside the owned nested curve getter."""
import json
import os

def run(doc, ids, case, host, options, identifier):
    import clr
    clr.AddReference('System.Core')
    from System.Linq.Expressions import Expression as E, ParameterExpression
    R,S=host['Rhino'],host['System'];G=R.Geometry
    source,index,start,end=case['edge_ranges'][0]
    obj=doc.Objects.FindId(ids[source]);brep=obj.Geometry;edge=brep.Edges[index]
    original=edge.DuplicateCurve()
    try:curve_type=original.GetType().Name
    finally:original.Dispose()
    parameters=[edge.Domain.ParameterAt(f) for f in [start,end]]
    points=[host['_xyz'](edge.PointAt(t)) for t in parameters]
    center=edge.PointAt(edge.Domain.Mid);normals=[]
    for face_index in edge.AdjacentFaces():
        face=brep.Faces[face_index]
        closest,u,v=face.ClosestPoint(center)
        if not closest:raise ValueError('owned edge face projection failed')
        n=face.NormalAt(u,v)
        if face.OrientationIsReversed:n.Reverse()
        normals.append(n)
    normal=G.Vector3d(*[sum(getattr(n,k) for n in normals) for k in ['X','Y','Z']])
    if not normal.Unitize():raise ValueError('owned edge has no viewing normal')
    view=doc.Views.ActiveView;vp=view.ActiveViewport;plane=G.Plane(brep.GetBoundingBox(True).Center,normal)
    if not vp.SetToPlanView(plane.Origin,plane.XAxis,plane.YAxis,True):raise ValueError('owned edge view failed')
    vp.DisplayMode=R.Display.DisplayModeDescription.FindByName('Shaded')
    bounds=brep.GetBoundingBox(True);bounds.Inflate(.5);vp.ZoomBoundingBox(bounds);view.Redraw();R.RhinoApp.Wait()
    pixel=vp.WorldToClient(center)
    from owned_brep_command import OwnedBrepCommand
    witness=OwnedBrepCommand(host);witness.ids=ids
    witness.verify_edge_pick(source,index,view,vp,int(pixel.X),int(pixel.Y))
    screen=view.ClientToScreen(S.Drawing.Point(int(pixel.X),int(pixel.Y)))
    root=os.path.dirname(host['__file__']);progress=os.path.join(root,'worker-progress.log');ack=os.path.join(root,'click-ack.json')
    marker='@component-click:%s:0:sub'%identifier;menu='@component-menu:%s-0:Second'%identifier
    state=S.Array[S.Int32]([0,1]);stamp=S.Array[S.DateTime]([S.DateTime.UtcNow]);started=S.DateTime.UtcNow
    array=lambda v:S.Array[E](v)
    integer=lambda v:E.Constant(S.Int32(v))
    item=lambda i:E.ArrayAccess(E.Constant(state),array([integer(i)]))
    method=lambda kind,name,types:clr.GetClrType(kind).GetMethod(name,S.Array[S.Type]([clr.GetClrType(t) for t in types]))
    call=lambda target,m,args:E.Call(target,m,array(args))
    block=lambda v:E.Block(array(v+[E.Empty()]))
    now=E.Property(None,clr.GetClrType(S.DateTime).GetProperty('UtcNow'))
    elapsed=lambda t:E.Property(call(now,method(S.DateTime,'Subtract',[S.DateTime]),[t]),'TotalSeconds')
    prompt=E.Property(None,clr.GetClrType(R.RhinoApp).GetProperty('CommandPrompt'))
    gate=call(prompt,method(S.String,'StartsWith',[S.String]),[E.Constant('Select curve')])
    def emit(name,x,y):return block([E.Assign(item(0),E.Add(item(0),integer(1))),E.Assign(E.ArrayAccess(E.Constant(stamp),array([integer(0)])),now),
        call(None,method(S.IO.File,'AppendAllText',[S.String,S.String]),[E.Constant(progress),E.Constant('\nPICK %s %d %d\n'%(name,x,y))])])
    action=E.IfThen(E.AndAlso(E.Equal(item(1),integer(1)),gate),E.IfThenElse(E.Equal(item(0),integer(0)),emit(marker,screen.X,screen.Y),
        E.IfThen(E.AndAlso(E.Equal(item(0),integer(1)),E.GreaterThan(elapsed(E.ArrayAccess(E.Constant(stamp),array([integer(0)]))),E.Constant(2.))),emit(menu,1,1))))
    timeout=E.IfThen(E.AndAlso(E.Equal(item(1),integer(1)),E.GreaterThan(elapsed(E.Constant(started)),E.Constant(35.))),
        block([E.Assign(item(1),integer(0)),call(None,method(S.IO.File,'AppendAllText',[S.String,S.String]),[E.Constant(progress),E.Constant('\nPICK_ABORT %s\n'%identifier)])]))
    invoke=method(R.RhinoApp,'InvokeOnUiThread',[S.Delegate,S.Array[S.Object]])
    def callback(body):
        managed=E.Lambda(clr.GetClrType(S.Action),body,S.Array[ParameterExpression]([])).Compile()
        p=E.Parameter(clr.GetClrType(S.Object),'state')
        return E.Lambda(clr.GetClrType(S.Threading.TimerCallback),block([call(None,invoke,[E.Constant(managed,clr.GetClrType(S.Delegate)),E.Constant(S.Array[S.Object]([]))])]),S.Array[ParameterExpression]([p])).Compile()
    # SetToPlanView also sets the construction plane. Endpoints are world data.
    tokens=['w'+','.join(repr(float(x)) for x in p) for p in points]
    numeric=None;confirmation=None;input_points=[list(p) for p in points]
    if case.get('edge_length'):
        numeric=edge.GetLength()*case['edge_length'];confirmation=points[1]
        total=edge.GetLength();prefix=edge.GetLength(G.Interval(edge.Domain.Min,parameters[0]))
        candidates=[]
        for forward in [True,False]:
            ok,parameter=edge.LengthParameter((prefix+(numeric if forward else -numeric))%total)
            if not ok:raise ValueError('owned numeric edge interval is unavailable')
            point=edge.PointAt(parameter)
            candidates.append((point.DistanceTo(G.Point3d(*confirmation)),[parameters[0],parameter] if forward else [parameter,parameters[0]]))
        parameters=candidates[0 if candidates[0][0]<candidates[1][0] else 1][1]
        points=[host['_xyz'](edge.PointAt(t)) for t in parameters]
    whole=' '.join('_SelID '+str(ids[i]) for i in case['whole'])
    script='_-ExtractPt '+' '.join(options)+' '
    if case.get('range_whole_first'):script+=whole+' '
    script+='_SubCrv _Pause '+tokens[0]+' '+(repr(float(numeric))+' ' if numeric is not None else '')+tokens[1]
    if not case.get('range_whole_first'):script+=' '+whole
    script+=' _Enter _Cancel _Cancel'
    whole_inputs=[['whole',i] for i in case['whole']]
    inputs=(whole_inputs+[['range',0]] if case.get('range_whole_first') else [['range',0]]+whole_inputs)
    doc.Objects.UnselectAll()
    timer=S.Threading.Timer(callback(action),None,100,100);limit=S.Threading.Timer(callback(timeout),None,500,500)
    try:returned=R.RhinoApp.RunScript(script,True)
    finally:state[1]=0;timer.Dispose();limit.Dispose()
    expected=menu if state[0]==2 else marker
    acknowledged=os.path.exists(ack) and json.loads(S.IO.File.ReadAllText(ack))==expected
    if state[0] not in [1,2] or not acknowledged:raise ValueError('owned edge click was not acknowledged')
    return returned,dict(parameters=[[source]+parameters],points=[[source]+points],edges=[index],
        script=script,marker=marker,menu=(state[0]==2),acknowledged=True,frustum_verified=True,world_coordinates=True,curve_type=curve_type,numeric_length=numeric,confirmation=confirmation,input_points=[[source]+input_points],inputs=inputs)
