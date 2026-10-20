"""Managed UI delegates drive bounded public getters while Python awaits RunScript."""
import json,os

def drive(op,host,guides,command="Conic"):
    if command not in ("Conic","Hyperbola","Helix","Spiral","Catenary","Ellipse","Circle","Arc","Reparameterize","Splop","ArrayCrvOnSrf","ArraySrf","ArrayCrv","OrientCrvToEdge","OffsetCrvOnSrf"):raise ValueError("unsupported managed curve getter command")
    R,S=host['Rhino'],host['System'];doc=R.RhinoDoc.ActiveDoc;view=doc.Views.ActiveView;vp=view.ActiveViewport
    plan_view=None
    if command in('ArrayCrvOnSrf','ArraySrf','ArrayCrv'):
        vp.SetProjection(R.Display.DefinedViewportProjection.Top,'Owned surface curve array input',False);vp.ZoomExtents();view.Redraw()
    if command=='OrientCrvToEdge':
        vp.SetProjection(R.Display.DefinedViewportProjection.Perspective,'Owned curve edge input',False);vp.ZoomExtents();view.Redraw()
    if command in('Circle','Arc','Splop','ArrayCrvOnSrf') and op.get('op')in('circle_tangent','arc_tangent','arc_extension','splop_tangent') and 'construction_plane'in op:
        origin,x,y=op['construction_plane']
        plan_view=bool(vp.SetToPlanView(R.Geometry.Point3d(*origin),R.Geometry.Vector3d(*x),R.Geometry.Vector3d(*y),True))
        if not plan_view:raise ValueError('could not align owned tangent input viewport')
        vp.ZoomExtents();view.Redraw()
    from snap_environment import environment
    import clr
    clr.AddReference('System.Core');clr.AddReference('System.Windows.Forms')
    from System.Linq.Expressions import Expression as E,ParameterExpression
    root=os.path.dirname(host['__file__']);progress=os.path.join(root,'worker-progress.log');ack=os.path.join(root,'click-ack.json')
    actions=op['actions'];state=S.Array[S.Int32]([0,1,0]);counts=S.Array[S.Int32]([0]*len(actions));observed=S.Array[S.String]([None]*len(actions));last=S.Array[S.DateTime]([S.DateTime.UtcNow]);started=S.DateTime.UtcNow
    def array(v):return S.Array[E](v)
    def integer(v):return E.Constant(S.Int32(v))
    def item(v,i):return E.ArrayAccess(E.Constant(v),array([integer(i)]))
    def method(t,name,types):return clr.GetClrType(t).GetMethod(name,S.Array[S.Type]([clr.GetClrType(v)for v in types]))
    def call(target,m,args):return E.Call(target,m,array(args))
    def block(v):return E.Block(array(v+[E.Empty()]))
    def compile_delegate(t,body,parameters=()):return E.Lambda(clr.GetClrType(t),body,S.Array[ParameterExpression](parameters)).Compile()
    def either(v):
        result=v[0]
        for x in v[1:]:result=E.OrElse(result,x)
        return result
    stage=item(state,0);active=E.Equal(item(state,1),integer(1));now=E.Property(None,clr.GetClrType(S.DateTime).GetProperty('UtcNow'))
    prompt=E.Property(None,clr.GetClrType(R.RhinoApp).GetProperty('CommandPrompt'))
    starts=method(S.String,'StartsWith',[S.String]);contains=method(S.String,'Contains',[S.String]);send=method(R.RhinoApp,'SendKeystrokes',[S.String,S.Boolean])
    write=method(S.IO.File,'WriteAllText',[S.String,S.String]);append=method(S.IO.File,'AppendAllText',[S.String,S.String]);exists=method(S.IO.File,'Exists',[S.String]);read=method(S.IO.File,'ReadAllText',[S.String]);move=method(S.IO.File,'Move',[S.String,S.String])
    subtract=method(S.DateTime,'Subtract',[S.DateTime]);elapsed=lambda t:E.Property(call(now,subtract,[t]),'TotalSeconds')
    def send_token(token):return call(None,send,[E.Constant(token),E.Constant(True)])
    def log(message):return call(None,append,[E.Constant(progress),E.Constant(message)])
    def point(p):
        if command in('ArrayCrvOnSrf','ArraySrf','ArrayCrv','OrientCrvToEdge','OffsetCrvOnSrf'):
            from number_token import number_token
            return 'w'+','.join(number_token(x)for x in p)
        return 'w'+','.join(repr(float(x))for x in p)
    prefixes=('Hyperbola','New A coefficient','New B coefficient')if command=='Hyperbola'else('Start of conic','End of conic','Apex','Curvature point or rho','Select curve','Start of curve','Current length')
    if command in ('Helix','Spiral'):prefixes=('Start of axis','End of axis','Radius and start point','Diameter and start point','First radius and start point','First diameter and start point','Second radius','Second diameter','Spiral center','Mode','Number of turns','New number of turns','Pitch','New pitch','Number of points per turn','Points per turn')
    if command=='Catenary':prefixes=('Start of catenary','End of catenary','Catenary','Direction','Point','Mode','Output','Number','Length','Parameter','Apex')
    if command=='Ellipse':prefixes=('Deformable point count','Select curve','Ellipse','Center','End','Start','First','Second','Corner','Opposite','Point','Distance','Radius','Focus','Degree','Number','Direction','Length')
    if command=='Circle':prefixes=('Degree','PointCount','Point','Deformable','Select curve','Circle','Center','First','Second','Third','Start','End','Radius','Diameter','Circumference','Area','Direction','Orientation')
    if command in('ArrayCrvOnSrf','ArraySrf','ArrayCrv','OrientCrvToEdge','OffsetCrvOnSrf'):prefixes=('Select objects to array','Base point','Select a curve on a surface','Select curve to temporarily shorten','Start of curve','Current length','Select the surface','Position objects or distance from last','Number of objects','Number of copies','Distance between copies')
    if command=='Splop':prefixes=('Center of reference sphere','First','Second','Third','Radius','Select surface','Pick splop','Pick point')
    if command=='Arc':prefixes=('Degree','PointCount','Point','Deformable','Select','Choose','Arc','Center','First','Second','Third','Start','End','Mid','Radius','Direction','Angle','Length','Distance')
    if command=='Reparameterize':prefixes=('Select curve','Select surface','Select object','Domain start','Domain end','U domain start','U domain end','V domain start','V domain end')
    base=either([call(prompt,starts,[E.Constant(p)])for p in prefixes])
    number_prompt=either([call(prompt,starts,[E.Constant(p)])for p in ('Domain start','Domain end','U domain start','U domain end','V domain start','V domain end')])if command=='Reparameterize'else base
    length=call(prompt,contains,[E.Constant('curve length')]);selection=call(prompt,starts,[E.Constant('Select surface to splop on' if command=='Splop' else 'Select curve')])
    def acknowledged(marker):return E.AndAlso(call(None,exists,[E.Constant(ack)]),E.Equal(call(None,read,[E.Constant(ack)]),E.Constant(json.dumps(marker))))
    branches=[];tokens=[];picks=[];hovers=[];mouse_steps=[];clicks=[]
    pointer_plane=vp.GetConstructionPlane().Plane
    arc_pointer_path=S.Array[S.Int32]([0]*384);arc_pointer_count=S.Array[S.Int32]([0])
    arc_pointer_initial=S.Array[S.Int32]([0]*(2*len(actions)))
    def prompt_gate(prefix):
        if command=='OrientCrvToEdge' and prefix=='Select curve':
            # Numeric SubCrv confirmation changes its text while retaining a
            # public point getter. Never confuse it with object selection.
            in_point=method(R.Input.RhinoGet,'InGetPoint',[R.RhinoDoc])
            return E.AndAlso(E.Equal(prompt,E.Constant(prefix)),call(None,in_point,[E.Constant(doc,clr.GetClrType(R.RhinoDoc))]))
        return call(prompt,starts,[E.Constant(prefix)])
    def pointer_request(i,marker,screen):
        receipt=[]
        if op.get('op')in('arc_command','arc_extension'):
            from System.Windows.Forms import Cursor
            position=E.Property(None,clr.GetClrType(Cursor).GetProperty('Position'))
            receipt=[E.Assign(item(arc_pointer_initial,2*i),E.Property(position,'X')),E.Assign(item(arc_pointer_initial,2*i+1),E.Property(position,'Y'))]
        return E.IfThen(E.Equal(item(counts,i),integer(0)),block([E.Assign(item(counts,i),integer(1))]+receipt+[log('\nPICK %s %d %d\n'%(marker,screen.X,screen.Y))]))
    for i,a in enumerate(actions):
        if command in('ArrayCrvOnSrf','ArraySrf','ArrayCrv','OrientCrvToEdge','OffsetCrvOnSrf'):base=either([prompt_gate(p)for p in op['action_prompts'][i]])
        advance=[E.Assign(item(observed,i),prompt),E.Assign(stage,integer(i+1)),E.Assign(item(last,0),now)]
        located=command in('Circle','Arc','Splop','ArrayCrvOnSrf','ArrayCrv','OrientCrvToEdge','OffsetCrvOnSrf')and op.get('op')in('circle_tangent','arc_tangent','arc_extension','splop_tangent','array_curve_surface','curve_array','orient_curve_edge')and isinstance(a,dict)and'curve'in a and'pick'in a
        if isinstance(a,dict)and('click'in a or located):
            aim=a['pick']if located else a['click'];pixel=vp.WorldToClient(R.Geometry.Point3d(*aim));screen=view.ClientToScreen(S.Drawing.Point(int(pixel.X),int(pixel.Y)))
            name=op['id']+'-'+str(i);marker='@catenary-click:'+name
            request=pointer_request(i,marker,screen)
            ready=os.path.join(root,'catenary-click-ready-'+name+'.json')
            body=E.IfThen(base,request)
            if command=='Reparameterize':
                selected=os.path.join(root,'block-selection-ready-'+name+'.json')
                accepted=E.AndAlso(call(None,exists,[E.Constant(selected)]),E.Equal(call(None,read,[E.Constant(selected)]),E.Constant(json.dumps(marker))))
                confirm=E.AndAlso(E.Equal(item(counts,i),integer(9)),E.AndAlso(selection,accepted))
                if i+1<len(actions)and isinstance(actions[i+1],dict)and'click'in actions[i+1]:
                    body=block([body,E.IfThen(confirm,block([E.Assign(stage,integer(i+1)),E.Assign(item(last,0),now)]))])
                else:
                    body=block([body,
                        E.IfThen(E.AndAlso(E.GreaterThanOrEqual(item(counts,i),integer(9)),number_prompt),block([E.Assign(stage,integer(i+1)),E.Assign(item(last,0),now)])),
                        E.IfThen(confirm,block([E.Assign(item(counts,i),integer(10)),E.Assign(item(last,0),now)]+([E.Assign(stage,integer(i+1))]if i+1==len(actions)else[])+[log('\nReparameterize accepts selection %s\n'%op['id']),send_token('')]))])
            mouse_steps.append((i,marker,pixel,ready,[],base))
            clicks.append(dict(action=i,marker=marker,aim=aim,pixel=[int(pixel.X),int(pixel.Y)]));tokens.append(None)
            if op.get('op')=='arc_command':
                tilt=actions.index('Tilted');center=[p for p in actions[:tilt]if isinstance(p,list)][0]
                plane=R.Geometry.Plane(R.Geometry.Point3d(*center),R.Geometry.Point3d(*actions[tilt+1]),R.Geometry.Point3d(*actions[tilt+2]))
                ray=vp.ClientToWorld(S.Drawing.Point(int(pixel.X),int(pixel.Y)))
                ok,t=R.Geometry.Intersect.Intersection.LinePlane(ray,plane)
                if ok:clicks[-1]['projected_point']=host['_xyz'](ray.PointAt(t))
            if op.get('op')=='arc_extension' and not located:
                selected=next(p for p in actions if isinstance(p,dict)and'curve'in p)
                source=doc.Objects.FindId(guides[selected['curve']]).Geometry
                if 'edge'in selected:source=source.Edges[selected['edge']]
                endpoint=source.PointAtEnd if source.PointAtEnd.DistanceTo(R.Geometry.Point3d(*selected['pick']))<source.PointAtStart.DistanceTo(R.Geometry.Point3d(*selected['pick']))else source.PointAtStart
                tangent=source.TangentAtEnd if endpoint==source.PointAtEnd else -source.TangentAtStart
                center=R.Geometry.Point3d(*actions[actions.index('Center')+1]);normal_plane=R.Geometry.Plane(endpoint,tangent);center=normal_plane.ClosestPoint(center)
                radial=endpoint-center;normal=R.Geometry.Vector3d.CrossProduct(radial,tangent)
                plane=R.Geometry.Plane(center,normal);pointer_plane=plane
                ray=vp.ClientToWorld(S.Drawing.Point(int(pixel.X),int(pixel.Y)))
                ok,t=R.Geometry.Intersect.Intersection.LinePlane(ray,plane)
                if ok:clicks[-1]['projected_point']=host['_xyz'](ray.PointAt(t))
            if located:
                clicks[-1]['curve']=a['curve'];clicks[-1]['edge']=a.get('edge')
                if 'edge'not in a or op.get('op')in('arc_extension','splop_tangent','array_curve_surface','curve_array','orient_curve_edge'):
                    source=doc.Objects.FindId(guides[a['curve']]).Geometry
                    if 'edge'in a:source=source.Edges[a['edge']]
                    ray=vp.ClientToWorld(S.Drawing.Point(int(pixel.X),int(pixel.Y)))
                    ok,t=R.Geometry.Intersect.Intersection.LinePlane(ray,vp.GetConstructionPlane().Plane)
                    if ok:
                        ok,parameter=source.ClosestPoint(ray.PointAt(t))
                        if ok:
                            accepted=source.PointAt(parameter)
                            clicks[-1]['projected_point']=[accepted.X,accepted.Y,accepted.Z]
                            clicks[-1]['projected_parameter']=float(parameter)
        elif isinstance(a,dict)and('hover'in a or'edge'in a):
            hover='hover'in a;aim=a['hover']if hover else a['pick'];pixel=vp.WorldToClient(R.Geometry.Point3d(*aim));screen=view.ClientToScreen(S.Drawing.Point(int(pixel.X),int(pixel.Y)))
            name=op['id']+'-'+str(i);marker=('@subcurve-direction:'if hover else'@conic-edge:')+name
            request=pointer_request(i,marker,screen)
            if hover:
                ready=os.path.join(root,'subcurve-direction-ready-'+name+'.json')
                hover_gate=base if op.get('op')=='arc_command'else length
                body=E.IfThen(hover_gate,request)
                mouse_steps.append((i,marker,pixel,ready,advance,hover_gate))
                hovers.append(dict(action=i,marker=marker,aim=aim,pixel=[int(pixel.X),int(pixel.Y)]))
                if op.get('op')=='arc_command':
                    tilt=actions.index('Tilted');center=[p for p in actions[:tilt]if isinstance(p,list)][0]
                    plane=R.Geometry.Plane(R.Geometry.Point3d(*center),R.Geometry.Point3d(*actions[tilt+1]),R.Geometry.Point3d(*actions[tilt+2]))
                    ray=vp.ClientToWorld(S.Drawing.Point(int(pixel.X),int(pixel.Y)))
                    ok,t=R.Geometry.Intersect.Intersection.LinePlane(ray,plane)
                    if ok:hovers[-1]['projected_point']=host['_xyz'](ray.PointAt(t))
            else:
                body=block([E.IfThen(selection,request),E.IfThen(acknowledged(marker),block(advance))]);picks.append(dict(action=i,marker=marker,pick=aim,authored_edge=a['edge']))
            tokens.append(None)
        else:
            if isinstance(a,dict)and'relative'in a:token='r'+','.join(repr(float(v))for v in a['relative']);gate=base
            elif isinstance(a,dict):token="'_-SelID "+str(guides[a['curve']]);gate=base if command in('ArrayCrvOnSrf','ArraySrf','ArrayCrv','OrientCrvToEdge','OffsetCrvOnSrf')else selection
            elif isinstance(a,list):token=point(a);gate=base
            elif isinstance(a,str):token='_'+a;gate=length if a in('D','Direction=Locked','Direction=Free')else base
            else:
                if command in('ArrayCrvOnSrf','ArraySrf','ArrayCrv','OrientCrvToEdge','OffsetCrvOnSrf'):
                    from number_token import number_token
                    token=number_token(a)
                else:token=repr(float(a))
                gate=base
            if command=='Reparameterize':
                gate=number_prompt
                if a=='Enter':token=''
            body=E.IfThen(gate,block(advance+[log('\n%s input %s %d: %s\n'%(command,op['id'],i,token)),send_token(token)]));tokens.append(token)
        branches.append(body)
    done=E.Label();body=[]
    for i,branch in enumerate(branches):body.append(E.IfThen(E.Equal(stage,integer(i)),block([branch,E.Goto(done)])))
    body.append(E.Label(done));ready=E.GreaterThan(elapsed(item(last,0)),E.Constant(.4))
    action=compile_delegate(S.Action,E.IfThen(E.AndAlso(active,ready),block(body)))
    cancel_input=send_token('_Cancel')
    cancel=compile_delegate(S.Action,E.IfThen(E.AndAlso(active,E.GreaterThan(elapsed(E.Constant(started)),E.Constant(45.))),block([E.Assign(item(state,2),integer(1)),cancel_input])))
    invoke=method(R.RhinoApp,'InvokeOnUiThread',[S.Delegate,S.Array[S.Object]])
    def callback(action):
        parameter=E.Parameter(clr.GetClrType(S.Object),'state');body=call(None,invoke,[E.Constant(action,clr.GetClrType(S.Delegate)),E.Constant(S.Array[S.Object]([]))]);return compile_delegate(S.Threading.TimerCallback,block([body]),[parameter])
    from managed_mouse_callback import create
    def mouse_handler(argument,delegate):
        event=E.Parameter(argument,'event');pixel=E.Property(event,'ViewportPoint');viewport=E.Property(E.Property(E.Property(event,'View'),'ActiveViewport'),'Id')
        same_view=E.Equal(viewport,E.Constant(vp.Id,clr.GetClrType(S.Guid)))
        handlers=[]
        if op.get('op')=='arc_command':
            index=item(arc_pointer_count,0)
            values=[stage,E.Property(pixel,'X'),E.Property(pixel,'Y')]
            slots=[E.ArrayAccess(E.Constant(arc_pointer_path),array([E.Add(E.Multiply(index,integer(3)),integer(k))]))for k in range(3)]
            handlers.append(E.IfThen(E.AndAlso(same_view,E.LessThan(index,integer(128))),block([E.Assign(slot,value)for slot,value in zip(slots,values)]+[E.Assign(index,E.Add(index,integer(1)))])))
        for i,marker,target,ready,advance,gate in mouse_steps:
            x=E.Property(pixel,'X');y=E.Property(pixel,'Y')
            near=E.AndAlso(E.AndAlso(E.GreaterThanOrEqual(x,integer(int(target.X)-1)),E.LessThanOrEqual(x,integer(int(target.X)+1))),E.AndAlso(E.GreaterThanOrEqual(y,integer(int(target.Y)-1)),E.LessThanOrEqual(y,integer(int(target.Y)+1))))
            condition=E.AndAlso(E.Equal(stage,integer(i)),E.AndAlso(E.Equal(item(counts,i),integer(1)),E.AndAlso(same_view,E.AndAlso(gate,near))))
            handlers.append(E.IfThen(condition,block([E.Assign(item(counts,i),integer(7)),call(None,write,[E.Constant(ready+'.tmp'),E.Constant(json.dumps(marker))]),call(None,move,[E.Constant(ready+'.tmp'),E.Constant(ready)])]+advance)))
        return E.Lambda(delegate,E.IfThen(active,block(handlers)),S.Array[ParameterExpression]([event])).Compile()
    mouse=create(mouse_handler,host)if mouse_steps else None
    def click_handler(argument,delegate):
        from System.Windows.Forms import MouseButtons
        event=E.Parameter(argument,'event');pixel=E.Property(event,'ViewportPoint');viewport=E.Property(E.Property(E.Property(event,'View'),'ActiveViewport'),'Id')
        same_view=E.Equal(viewport,E.Constant(vp.Id,clr.GetClrType(S.Guid)))
        left=E.Equal(E.Property(event,'Button'),E.Constant(MouseButtons.Left,clr.GetClrType(MouseButtons)))
        handlers=[]
        for c in clicks:
            i=c['action'];x,y=c['pixel']
            gate=either([prompt_gate(p)for p in op['action_prompts'][i]])if command in('ArrayCrvOnSrf','ArraySrf','ArrayCrv','OrientCrvToEdge','OffsetCrvOnSrf')else base
            near=E.AndAlso(E.AndAlso(E.GreaterThanOrEqual(E.Property(pixel,'X'),integer(x-1)),E.LessThanOrEqual(E.Property(pixel,'X'),integer(x+1))),E.AndAlso(E.GreaterThanOrEqual(E.Property(pixel,'Y'),integer(y-1)),E.LessThanOrEqual(E.Property(pixel,'Y'),integer(y+1))))
            condition=E.AndAlso(E.Equal(stage,integer(i)),E.AndAlso(E.Equal(item(counts,i),integer(7)),E.AndAlso(same_view,E.AndAlso(gate,E.AndAlso(near,E.AndAlso(left,E.Not(E.Property(event,'Cancel'))))))))
            advance=[E.Assign(item(observed,i),prompt),E.Assign(item(counts,i),integer(9)),E.Assign(item(last,0),now)]
            if command!='Reparameterize':advance.append(E.Assign(stage,integer(i+1)))
            handlers.append(E.IfThen(condition,block(advance)))
        return E.Lambda(delegate,E.IfThen(active,block(handlers)),S.Array[ParameterExpression]([event])).Compile()
    click_mouse=create(click_handler,host,'OnMouseDown')if clicks else None
    if click_mouse is not None:click_mouse.Enabled=True
    if mouse is not None:mouse.Enabled=True
    timer=S.Threading.Timer(callback(action),None,100,100);timeout=S.Threading.Timer(callback(cancel),None,1000,500)
    try:
        with environment(dict(persistent_snaps=op.get('persistent_snaps',[])if command=='Catenary'else[]),host):
            aid=R.ApplicationSettings.ModelAidSettings
            snap_settings=dict(enabled=bool(aid.Osnap),modes=int(aid.OsnapModes),project_to_cplane=bool(aid.ProjectSnapToCPlane),capture_radius=int(aid.OsnapPickboxRadius))
            returned=R.RhinoApp.RunScript(('_-ArrayCrv'if command=='ArrayCrv'else '_'+command)+' _Pause',True)
    finally:
        state[1]=0;timer.Dispose();timeout.Dispose()
        if mouse is not None:mouse.Enabled=False
        if click_mouse is not None:click_mouse.Enabled=False
    errors=['owned '+command+' input timeout']if state[2]else[]
    if state[0]!=len(actions):errors.append('incomplete owned '+command+' input')
    trace=dict(completed=int(state[0]),expected=len(actions),errors=errors,prompts=list(observed),sent=[dict(action=i,token=tokens[i],prompt=observed[i])for i in range(state[0])if tokens[i]is not None],motion=[dict(h,acknowledged=counts[h['action']]>6)for h in hovers],edge_picks=[dict(p,acknowledged=observed[p['action']]is not None)for p in picks],motion_acknowledgement='No mouse input was requested; public RhinoApp.SendKeystrokes delivered typed tokens.'if command!='Conic'else'Rhino MouseCallback.OnEndMouseMove in the owned active viewport, matching the requested source projection; the callback is a managed delegate.')

    if plan_view is not None:trace['aligned_construction_plane_view']=plan_view
    if clicks:
        trace['clicks']=[dict(c,motion_acknowledged=counts[c['action']]>=7,click_acknowledged=counts[c['action']]>=9)for c in clicks]
        if command=='Reparameterize':trace['selection_enter_sent']=any(counts[c['action']]==10 for c in clicks)
        trace['snap_settings']=snap_settings
        if op.get('op')=='arc_command':trace['pointer_path']=[dict(action=int(arc_pointer_path[3*i]),pixel=[int(arc_pointer_path[3*i+1]),int(arc_pointer_path[3*i+2])])for i in range(arc_pointer_count[0])]
        if op.get('op')in('arc_command','arc_extension'):
            initial_plane=pointer_plane if op.get('op')=='arc_extension'else plane
            for c in trace['clicks']+trace['motion']:
                i=c['action'];initial=view.ScreenToClient(S.Drawing.Point(arc_pointer_initial[2*i],arc_pointer_initial[2*i+1]))
                ray=vp.ClientToWorld(initial);ok,t=R.Geometry.Intersect.Intersection.LinePlane(ray,initial_plane)
                c['initial_pixel']=[int(initial.X),int(initial.Y)]
                if ok:c['initial_projected_point']=host['_xyz'](ray.PointAt(t))
        trace['motion_acknowledgement']='Public Rhino MouseCallback.OnEndMouseMove in the owned viewport precedes each public OnMouseDown acknowledgement of a physical left click.'
    return bool(returned),trace
