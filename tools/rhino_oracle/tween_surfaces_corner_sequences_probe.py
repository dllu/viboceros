"""Closed, owned public TweenSurfaces command recipes and complete surface data."""
import re


def surface(z, degree=1, count=2, rational=False, shifted=False):
    knots = [0.]*(degree+1) + ([.5] if count > degree+1 else []) + [1.]*(degree+1)
    if shifted:
        knots_u, knots_v = [2.+3.*x for x in knots], [-4.+6.*x for x in knots]
    else:
        knots_u = knots_v = knots
    return dict(degree_u=degree, degree_v=degree, control_point_count_u=count, control_point_count_v=count,
                knots_u=knots_u, knots_v=knots_v,
                control_points=[dict(point=[4.*u/(count-1), 6.*v/(count-1), z+(u*v if degree>1 else 0.)],
                                     weight=1.+.25*(u+v) if rational else 1.)
                                for v in range(count) for u in range(count)])


def patch(x,z,warp):
    d=surface(z)
    for i,c in enumerate(d['control_points']):
        c['point'][0]+=x
        if i==3:c['point'][2]+=warp
    return d
SEQUENCES={'baseline':[], 'swap_twice':[0,0], 'u_origin':[1,1], 'v_origin':[2,2], 'u_restore':[1,0], 'v_restore':[2,0], 'swap_original_u':[0,1], 'swap_original_v':[0,2], 'u_inactive':[1,2], 'v_inactive':[2,1], 'u_v':[1,3], 'v_u':[2,3], 'u_v_swap':[1,3,3], 'swap_u_v':[0,2,3]}
SPECS={name:dict(sources=[patch(0.,0.,1.),patch(10.,4.,3.)],method='None',number=1,sample=4,layer='CurrentLayer',clicks=[dict(source=1,corner=c)for c in sequence])for name,sequence in SEQUENCES.items()}


def request():
    return dict(protocol_version=1, iterations=1, operations=[dict(op='tween_surfaces_corner_sequences',id='tween_srf_'+case,case=case) for case in SPECS])


def validate_request(q):
    if (not isinstance(q,dict) or set(q)-{'protocol_version','iterations','operations'}
            or type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1) != 1
            or not isinstance(q.get('operations'),list) or not 1<=len(q['operations'])<=32):
        raise ValueError('TweenSurfaces requires bounded protocol 1 recipes')
    seen=set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op)!={'op','id','case'} or op['op']!='tween_surfaces_corner_sequences'
                or not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None
                or op['id'] in seen or not isinstance(op['case'],str) or op['case'] not in SPECS):
            raise ValueError('invalid TweenSurfaces recipe')
        seen.add(op['id'])


def run(op, host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino,System=host['Rhino'],host['System']
    if Rhino.Commands.Command.InCommand():raise ValueError('TweenSurfaces requires idle execution')
    doc,G=Rhino.RhinoDoc.ActiveDoc,Rhino.Geometry
    if list(doc.Objects):raise ValueError('TweenSurfaces requires an empty owned document')
    from join_probe import observe_command
    from shrink_face_input import _input_hooks
    from snap_environment import environment
    import clr,json,os
    clr.AddReference('System.Windows.Forms')
    from System.Windows.Forms import Timer
    view=doc.Views.ActiveView;vp=view.ActiveViewport
    original=Rhino.DocObjects.ViewportInfo(vp);saved_plane=vp.GetConstructionPlane()
    timer=Timer();timer.Interval=100;hooks_owned=False
    motion=[];click_trace=[];sent=[False];ready=[False];ticks=[0]
    marker='@tween-corner:'+op['id']
    target=[None];pixel=[None];click_index=[0];calibrations=[];last_click=[System.DateTime.UtcNow]
    saved_layer,saved_tolerance=doc.Layers.CurrentLayerIndex,doc.ModelAbsoluteTolerance
    doc.ModelAbsoluteTolerance=1e-6
    spec=SPECS[op['case']]
    ids,layers,groups=[],[],[]

    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda x:x.RuntimeSerialNumber):
            g,a=obj.Geometry,obj.Attributes
            s=g.Faces[0].UnderlyingSurface() if isinstance(g,G.Brep) else g
            rows.append(dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                name=a.Name,layer=layers.index(a.LayerIndex) if a.LayerIndex in layers else None,
                selected=bool(obj.IsSelected(False)),groups=sorted(groups.index(i) for i in (a.GetGroupList() or []) if i in groups),
                color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],
                attribute_text=a.GetUserString('Code'),geometry_text=g.GetUserString('Code'),
                valid=bool(g.IsValid),faces=g.Faces.Count if isinstance(g,G.Brep) else 1,
                definition=host['_nurbs_surface_definition'](s),
                samples=[host['_xyz'](s.PointAt(s.Domain(0).ParameterAt(u/8.),s.Domain(1).ParameterAt(v/8.))) for v in range(9) for u in range(9)]))
        return rows

    def invoke(macro,command):
        marker='Viboceros TweenSurfaces '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker);host['_record_progress'](op['id']+' '+macro)
        success,after,events=observe_command(Rhino.Commands.Command,command,lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        active=bool(Rhino.Commands.Command.InCommand())
        if active:Rhino.RhinoApp.RunScript('!',False)
        return dict(success=success,after=after,after_script=snapshot(),events=events,active=active,
                    history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1],macro=macro)

    try:
        for i in range(3):
            layer=Rhino.DocObjects.Layer();layer.Name='Viboceros TweenSurface '+str(System.Guid.NewGuid())
            try:layers.append(doc.Layers.Add(layer))
            finally:layer.Dispose()
        for i,definition in enumerate(spec['sources']):
            g=host['_nurbs_surface_from_definition'](definition);a=Rhino.DocObjects.ObjectAttributes()
            try:
                a.Name,a.LayerIndex='source-'+str(i),layers[i]
                a.ObjectColor=System.Drawing.Color.FromArgb(20+i,40,60);a.ColorSource=Rhino.DocObjects.ObjectColorSource.ColorFromObject
                a.SetUserString('Code','attribute-'+str(i));g.SetUserString('Code','geometry-'+str(i))
                ids.append(doc.Objects.AddSurface(g,a))
                if ids[-1]==System.Guid.Empty:raise ValueError('TweenSurfaces insertion failed')
            finally:g.Dispose();a.Dispose()
        for members in [[key] for key in ids]+[ids]:groups.append(doc.Groups.Add('Viboceros TweenSurface '+str(System.Guid.NewGuid()),members))
        doc.Layers.SetCurrentLayerIndex(layers[2],True);doc.ClearUndoRecords(True)
        before=snapshot()
        macro='_TweenSurfaces '+' '.join('_SelID '+str(key) for key in ids)+' _NumberOfSurfaces='+str(spec['number'])+' _MatchMethod=_'+spec['method']
        if spec['method']=='SamplePoints':macro+=' _SampleNumber='+str(spec['sample'])
        macro+=' _OutputLayer=_'+spec['layer']
        vp.SetProjection(Rhino.Display.DefinedViewportProjection.Top,'Owned TweenSurfaces corners',False)
        vp.SetConstructionPlane(G.Plane.WorldXY)
        vp.ZoomBoundingBox(G.BoundingBox(G.Point3d(-2.,-2.,-1.),G.Point3d(16.,8.,9.)))
        def request_click(index):
            click=spec['clicks'][index]
            target[0]=host['_point'](spec['sources'][click['source']]['control_points'][click['corner']]['point'])
            pixel[0]=vp.WorldToClient(target[0])
            screen=view.ClientToScreen(System.Drawing.Point(int(pixel[0].X),int(pixel[0].Y)))
            name=marker+'-'+str(index)
            calibrations.append(__import__('viewport_capture').capture(vp,target[0],[int(pixel[0].X),int(pixel[0].Y)],host))
            host['_record_progress']('MOVE %s %d %d'%(name,screen.X,screen.Y))
        macro+=' _Pause' if spec['clicks'] else ' _Enter'
        def tick(sender,event):
            ticks[0]+=1
            if ticks[0]>600:Rhino.RhinoApp.RunScript('!',False)
        class Mouse(Rhino.UI.MouseCallback):
            def OnEndMouseMove(self,event):
                if event.View.ActiveViewport.Id==vp.Id and len(motion)<32:motion.append([int(event.ViewportPoint.X),int(event.ViewportPoint.Y)])
            def OnEndMouseDown(self,event):
                if event.View.ActiveViewport.Id!=vp.Id:return
                click_trace.append(dict(pixel=[int(event.ViewportPoint.X),int(event.ViewportPoint.Y)],prompt=Rhino.RhinoApp.CommandPrompt));last_click[0]=System.DateTime.UtcNow
                path=os.path.join(os.path.dirname(host['__file__']),'corner-sequence-clicked-'+op['id']+'-'+str(len(click_trace)-1)+'.json')
                with open(path+'.tmp','w')as f:json.dump(marker+'-'+str(len(click_trace)-1),f)
                os.rename(path+'.tmp',path)
        timer.Tick+=tick
        sampled=[]
        sources=[host['_nurbs_surface_from_definition'](definition) for definition in spec['sources']]
        try:
            sdk=G.Surface.CreateTweenSurfacesWithSampling(sources[0],sources[1],spec['number'],spec['sample'],doc.ModelAbsoluteTolerance)
            if sdk:
                for output in sdk:
                    try:sampled.append(host['_nurbs_surface_definition'](output))
                    finally:output.Dispose()
        finally:
            for source in sources:source.Dispose()
        for index in range(len(spec['clicks'])):request_click(index)
        hooks_owned=True
        with environment(dict(persistent_snaps=[]),host):
            with _input_hooks(timer,Mouse(),[]):
                command=invoke(macro,'TweenSurfaces')
        result=dict(case=op['case'],spec=spec,before=before,sampling_sdk=sampled,command=command,
                    motion=motion,click_trace=click_trace,click_pixel=[int(pixel[0].X),int(pixel[0].Y)] if pixel[0] else None,
                    projection=host['_xyz'](target[0]) if target[0] else None,
                    calibrations=calibrations)
        result['undo']=invoke('_Undo','Undo');result['redo']=invoke('_Redo','Redo')
        return result,0
    finally:
        if not hooks_owned:timer.Dispose()
        vp.SetViewProjection(original,False);vp.SetConstructionPlane(saved_plane);original.Dispose()
        if Rhino.Commands.Command.InCommand():Rhino.RhinoApp.RunScript('!',False)
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        for index in groups:doc.Groups.Delete(index)
        doc.Layers.SetCurrentLayerIndex(saved_layer,True)
        for index in reversed(layers):doc.Layers.Delete(index,True)
        doc.ModelAbsoluteTolerance=saved_tolerance
