"""Authored FlowAlongSrf point maps and public command observations."""
import math,re
if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_source
else:
    from geometry_validity_probe import validate as validate_source


def validate(op):
    allowed={'op','id','base','target','points','base_uv','target_uv','normal','command','source','options','selected','edit','group','plane','mouse','base_trim','target_trim','base_reversed','target_reversed','peer'}
    if not isinstance(op,dict)or op.get('op')!='surface_flow'or set(op)-allowed or not {'id','base','target','points'}<=set(op):raise ValueError('invalid surface Flow recipe')
    if not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,100}\Z',op['id'])is None:raise ValueError('invalid surface Flow id')
    for role in ('base','target'):
        if not isinstance(op[role],dict)or op[role].get('type')!='surface':raise ValueError('surface Flow requires NURBS surfaces')
        validate_source(dict(op='geometry_validity',id=op['id'],source=op[role]))
    def vector(p,count):
        return isinstance(p,list)and len(p)==count and all(type(v)in(int,float)and not(math.isnan(v)or math.isinf(v))for v in p)
    if not isinstance(op['points'],list)or not 1<=len(op['points'])<=2048 or any(not vector(p,3)for p in op['points']):raise ValueError('invalid surface Flow points')
    if 'plane'in op and(not isinstance(op['plane'],list)or len(op['plane'])not in(2,3)or any(not vector(p,3)for p in op['plane'])):raise ValueError('invalid surface Flow base plane')
    for role in ('base','target'):
        trim=op.get(role+'_trim')
        if trim is not None and(not isinstance(trim,list)or len(trim)!=2 or any(not vector(p,2)or p[0]>=p[1]for p in trim)):raise ValueError('invalid surface Flow trim bounds')
    selected=op.get('selected',[])
    if not isinstance(selected,list)or len(selected)>64 or any(type(i)is not int or not 0<=i<64 for i in selected)or len(set(selected))!=len(selected):raise ValueError('invalid surface Flow grips')
    if 'source'in op:
        validate_source(dict(op='geometry_validity',id=op['id'],source=op['source']))
        if op['source'].get('type')not in('point_cloud','nurbs','surface','mesh'):raise ValueError('unsupported surface Flow owner')
    if selected and op.get('source',{}).get('type')not in('nurbs','surface','mesh'):raise ValueError('surface Flow grips need a grip owner')
    if op.get('edit')and(not selected or op.get('source',{}).get('type')!='nurbs'):raise ValueError('surface Flow edit grips need a curve')
    for name in ('base_uv','target_uv'):
        if name in op and not vector(op[name],2):raise ValueError('invalid surface Flow pick')
    if ('base_uv'in op)!=('target_uv'in op):raise ValueError('surface Flow needs both orientation picks')
    if 'normal'in op and(not vector(op['normal'],3)or not any(op['normal'])):raise ValueError('invalid surface Flow normal')
    for flag in ('command','edit','group','mouse','base_reversed','target_reversed','peer'):
        if type(op.get(flag,False))is not bool:raise ValueError('invalid surface Flow flag')
    if op.get('mouse')and(not op.get('command')or 'base_uv'not in op or 'plane'in op):raise ValueError('surface Flow mouse capture requires two surface picks')
    options=op.get('options',{})
    if not isinstance(options,dict)or set(options)-{'copy','rigid','rigid_groups','auto_adjust','preserve_structure'}or any(type(v)is not bool for v in options.values()):raise ValueError('invalid surface Flow options')
    if op.get('command')and'source'in op:validate_source(dict(op='geometry_validity',id=op['id'],source=op['source']))


def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('surface Flow capture requires one iteration')
    R=host['Rhino'];G=R.Geometry
    surfaces=[host['_object_source'](op[role],tolerance)for role in('base','target')]
    try:
        if 'base_uv'in op:
            morph=G.Morphs.SporphSpaceMorph(surfaces[0],surfaces[1],G.Point2d(*op['base_uv']),G.Point2d(*op['target_uv']))
        else:morph=G.Morphs.SporphSpaceMorph(*surfaces)
        try:
            if 'normal'in op:morph.ConstrainNormal=G.Vector3d(*op['normal'])
            points=[host['_xyz'](morph.MorphPoint(G.Point3d(*p)))for p in op['points']]
            result=dict(sdk_points=points,valid=bool(morph.IsValid))
        finally:morph.Dispose()
        if op.get('command'):result['command']=run_command(op,host,tolerance)
        return result,0
    finally:
        for surface in surfaces:surface.Dispose()


def run_command(op,host,tolerance):
    from join_probe import observe_command
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;o=op.get('options',{})
    before=set(obj.Id for obj in doc.Objects);groups=set(g.Index for g in doc.Groups if not g.IsDeleted)
    surfaces=[host['_object_source'](op[role],tolerance)for role in('base','target')]
    geometry=host['_object_source'](op.get('source',dict(type='point_cloud',points=op['points'])),tolerance)
    try:
        guides=[]
        for role,surface in zip(('base','target'),surfaces):
            if role=='base'and 'plane'in op:continue
            trim=op.get(role+'_trim');reversed_=op.get(role+'_reversed',False)
            if trim is not None:
                (u0,u1),(v0,v1)=trim
                uv=G.PolylineCurve([G.Point3d(u,v,0.)for u,v in [(u0,v0),(u1,v0),(u1,v1),(u0,v1),(u0,v0)]])
                brep=G.Brep.CreateFromSurface(surface);edge=surface.Pushup(uv,1e-8)
                if edge is None:raise ValueError('surface Flow trim pushup failed')
                split=brep.Faces[0].Split([edge],1e-6)
                if split is None:raise ValueError('surface Flow trim split failed')
                candidates=[f.DuplicateFace(False)for f in split.Faces]
                def area(b):
                    mp=G.AreaMassProperties.Compute(b)
                    try:return mp.Area
                    finally:mp.Dispose()
                guide=min(candidates,key=area)
                for b in candidates:
                    if b is not guide:b.Dispose()
                for item in(uv,edge,split,brep):item.Dispose()
            elif reversed_:guide=G.Brep.CreateFromSurface(surface)
            else:guide=None
            if guide is None:guides.append(doc.Objects.AddSurface(surface))
            else:
                try:
                    if reversed_:guide.Flip()
                    if not guide.IsValid:raise ValueError('surface Flow reference invalid')
                    guides.append(doc.Objects.AddBrep(guide))
                finally:guide.Dispose()
        source=doc.Objects.Add(geometry)
        peer=None
        if op.get('peer'):
            cloud=G.PointCloud([G.Point3d(p[0]+4,p[1]+1,p[2]+2)for p in op['points']])
            try:peer=doc.Objects.AddPointCloud(cloud)
            finally:cloud.Dispose()
        if op.get('group'):doc.Groups.Add('Owned FlowAlongSrf',[source]+([peer]if peer else[]))
        def definition(key):
            g=doc.Objects.FindId(key).Geometry
            return dict(surface=host['_nurbs_surface_definition'](g.Faces[0].UnderlyingSurface().ToNurbsSurface()),reversed=bool(g.Faces[0].OrientationIsReversed),crc=int(g.DataCRC(0)))
        originals={key:definition(key)for key in guides};doc.Objects.UnselectAll()
        if op.get('selected'):
            obj=doc.Objects.FindId(source)
            if op.get('edit'):
                doc.Objects.Select(source,True);R.RhinoApp.RunScript('_EditPtOn',True);doc.Objects.UnselectAll();obj=doc.Objects.FindId(source)
            else:obj.GripsOn=True
            grips=obj.GetGrips()or[]
            if any(i>=len(grips)for i in op['selected']):raise ValueError('FlowAlongSrf grip index missing')
            for i in op['selected']:grips[i].Select(True)
        else:doc.Objects.Select(source,True)
        if peer:doc.Objects.Select(peer,True)
        def snapshot():
            outputs=[];objects=[obj for obj in doc.Objects if obj.Id not in before];active=sorted(g.Index for g in doc.Groups if not g.IsDeleted and g.Index not in groups)
            for obj in objects:
                if obj.Id in originals:continue
                g=obj.Geometry;row=dict(original=obj.Id==source,peer=obj.Id==peer,selected=bool(obj.IsSelected(False)),group_count=obj.GroupCount,groups=sorted(active.index(i)for i in(obj.GetGroupList()or[])if i in active),grips_on=bool(obj.GripsOn),grips=[dict(index=int(p.Index),point=host['_xyz'](p.CurrentLocation),selected=bool(p.IsSelected(False)))for p in(obj.GetGrips()or[])])
                if isinstance(g,G.PointCloud):row['points']=[host['_xyz'](p.Location)for p in g]
                elif isinstance(g,G.Curve):row['curve']=host['_nurbs_curve_definition'](g.ToNurbsCurve())
                elif isinstance(g,G.Brep)and g.Faces.Count==1:row['surface']=host['_nurbs_surface_definition'](g.Faces[0].UnderlyingSurface().ToNurbsSurface())
                elif isinstance(g,G.Mesh):row['mesh']=host['_polygon_mesh_value'](g)
                else:raise ValueError('unexpected FlowAlongSrf output')
                outputs.append(row)
            return dict(outputs=outputs,object_count=len(objects),guide_unchanged=all(definition(k)==v for k,v in originals.items()),guide_selected=any(bool(doc.Objects.FindId(k).IsSelected(False))for k in originals),group_count=len(active))
        doc.ClearUndoRecords(True);seed=snapshot()
        flags=' '.join('_'+name+'=_'+('Yes'if o.get(field,default)else'No')for name,field,default in [('Copy','copy',True),('Rigid','rigid',False),('AutoAdjust','auto_adjust',False),('PreserveStructure','preserve_structure',False)])
        if o.get('rigid'):flags+=' _RigidGroups=_'+('Yes'if o.get('rigid_groups',True)else'No')
        flags+=' _ConstrainNormal=_'+('Yes'if 'normal'in op else'No')
        point=lambda p:'w'+','.join(repr(float(v))for v in p)
        macro='_FlowAlongSrf '+flags
        if 'plane'in op:macro+=' _Plane '+('_3Point 'if len(op['plane'])==3 else'')+' '.join(point(p)for p in op['plane'])
        macro+=(' _Pause _Pause'if op.get('mouse')else''.join(" '_-SelID "+str(key)for key in guides))
        if 'normal'in op:macro+=' '+point([0,0,0])+' '+point(op['normal'])
        marker='Owned FlowAlongSrf '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker);host['_record_progress'](op['id']+' macro: '+macro)
        interaction=None
        if op.get('mouse'):
            (success,after,events),interaction=invoke_with_picks(op,host,surfaces,macro,snapshot)
        else:success,after,events=observe_command(R.Commands.Command,'FlowAlongSrf',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        result=dict(before=seed,after=after,after_script=snapshot(),events=events,command_returned=bool(success),macro=macro,history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip())
        if interaction is not None:result['interaction']=interaction
        result['undo_command_returned']=bool(R.RhinoApp.RunScript('_Undo',False));result['undo']=snapshot()
        result['redo_command_returned']=bool(R.RhinoApp.RunScript('_Redo',False));result['redo']=snapshot()
        return result
    finally:
        for obj in list(doc.Objects):
            if obj.Id not in before:
                if obj.GripsOn:obj.GripsOn=False
                doc.Objects.Delete(obj.Id,True)
        for g in list(doc.Groups):
            if not g.IsDeleted and g.Index not in groups:doc.Groups.Delete(g.Index)
        geometry.Dispose()
        for s in surfaces:s.Dispose()


def invoke_with_picks(op,host,surfaces,macro,snapshot):
    # Managed delegates keep the native getter live while IronPython runs the macro.
    from join_probe import observe_command
    from snap_environment import environment
    import clr,os
    clr.AddReference('System.Core')
    from System.Linq.Expressions import Expression as E, ParameterExpression
    R=host['Rhino'];S=host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    view=doc.Views.ActiveView;vp=view.ActiveViewport;original=R.DocObjects.ViewportInfo(vp);plane=vp.GetConstructionPlane()
    vp.SetProjection(R.Display.DefinedViewportProjection.Top,'Owned FlowAlongSrf picks',False);vp.SetConstructionPlane(G.Plane.WorldXY)
    box=surfaces[0].GetBoundingBox(True);box.Union(surfaces[1].GetBoundingBox(True));box.Inflate(box.Diagonal.Length*.1);vp.ZoomBoundingBox(box);view.Redraw()
    def array(values):return S.Array[E](values)
    def integer(v):return E.Constant(S.Int32(v))
    def item(values,i):return E.ArrayAccess(values,array([integer(i)]))
    def call(target,method,args):return E.Call(target,method,array(args))
    def block(values):return E.Block(array(values+[E.Empty()]))
    def method(t,name,types):return clr.GetClrType(t).GetMethod(name,S.Array[S.Type]([clr.GetClrType(v)for v in types]))
    def delegate(t,body,parameters=()):return E.Lambda(clr.GetClrType(t),body,S.Array[ParameterExpression](parameters)).Compile()
    state=S.Array[S.Int32]([0,1]);indices=E.Constant(state);stage=item(indices,0)
    prompt=E.Property(None,clr.GetClrType(R.RhinoApp).GetProperty('CommandPrompt'))
    contains=method(S.String,'Contains',[S.String]);append=method(S.IO.File,'AppendAllText',[S.String,S.String])
    progress=os.path.join(os.path.dirname(host['__file__']),'worker-progress.log');requests=[];actions=[]
    for i,role in enumerate(('base','target')):
        point=surfaces[i].PointAt(*op[role+'_uv']);pixel=vp.WorldToClient(point);screen=view.ClientToScreen(S.Drawing.Point(int(round(pixel.X)),int(round(pixel.Y))))
        requests.append(dict(role=role,point=host['_xyz'](point),pixel=[screen.X,screen.Y]))
        marker='\nPICK surface-flow-%s-%s %d %d\n'%(op['id'],role,screen.X,screen.Y)
        gate=E.AndAlso(E.Equal(stage,integer(i)),call(prompt,contains,[E.Constant(role.capitalize()+' surface')]))
        actions.append(E.IfThen(gate,block([E.Assign(stage,integer(i+1)),call(None,append,[E.Constant(progress),E.Constant(marker)])])))
    active=E.Equal(item(indices,1),integer(1));action=delegate(S.Action,E.IfThen(active,block(actions)))
    send=method(R.RhinoApp,'SendKeystrokes',[S.String,S.Boolean]);cancel=delegate(S.Action,E.IfThen(active,block([call(None,send,[E.Constant('_Cancel'),E.Constant(True)])])))
    invoke=method(R.RhinoApp,'InvokeOnUiThread',[S.Delegate,S.Array[S.Object]])
    def callback(action):
        arg=E.Parameter(clr.GetClrType(S.Object),'state')
        return delegate(S.Threading.TimerCallback,block([call(None,invoke,[E.Constant(action,clr.GetClrType(S.Delegate)),E.Constant(S.Array[S.Object]([]))])]),[arg])
    timer=S.Threading.Timer(callback(action),None,100,100);timeout=S.Threading.Timer(callback(cancel),None,30000,500)
    try:
        with environment(dict(persistent_snaps=[]),host):
            result=observe_command(R.Commands.Command,'FlowAlongSrf',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        if state[0]!=2:raise ValueError('surface Flow input incomplete')
        return result,dict(requests=requests,completed=int(state[0]))
    finally:
        state[1]=0;timer.Dispose();timeout.Dispose();vp.SetViewProjection(original,False);vp.SetConstructionPlane(plane);original.Dispose()
