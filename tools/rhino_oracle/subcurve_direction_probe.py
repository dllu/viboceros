"""Owned mouse direction locking through the public SubCrv getter."""
import json,os,re
CASES=('forward_point','backward_point','forward_numeric','backward_numeric',
       'closed_forward','closed_backward','unlock_point','direction_option','inline_forward','mark_forward',
       'mark_numeric','inline_numeric','closed_forward_numeric','closed_backward_numeric','same_side_point','closed_circle_numeric',
       'closed_circle_early_numeric','closed_early_backward_numeric')
def request():return dict(protocol_version=1,iterations=1,operations=[dict(op='subcurve_direction',id='direction_'+c,case=c) for c in CASES])
def validate_request(q):
    if (not isinstance(q,dict) or type(q.get('protocol_version')) is not int or q['protocol_version']!=1 or type(q.get('iterations',1)) is not int or q.get('iterations',1)!=1 or not isinstance(q.get('operations'),list) or not 1<=len(q['operations'])<=32):raise ValueError('direction locking requires bounded protocol 1 recipes')
    seen=set()
    for op in q['operations']:
        if not isinstance(op,dict) or set(op)!={'op','id','case'} or op['op']!='subcurve_direction' or not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None or op['id'] in seen or op.get('case') not in CASES:raise ValueError('invalid direction locking recipe')
        seen.add(op['id'])
def run(op,host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino,System=host['Rhino'],host['System'];G=Rhino.Geometry;doc=Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand():raise ValueError('direction locking requires idle execution')
    if list(doc.Objects):raise ValueError('direction locking requires empty owned document')
    from join_probe import observe_command
    from shrink_face_input import _input_hooks
    from snap_environment import environment
    import clr
    clr.AddReference('System.Windows.Forms')
    from System.Windows.Forms import Timer
    view=doc.Views.ActiveView;vp=view.ActiveViewport;original=Rhino.DocObjects.ViewportInfo(vp);plane=vp.GetConstructionPlane();saved=doc.ModelAbsoluteTolerance;doc.ModelAbsoluteTolerance=1e-6
    timer=Timer();timer.Interval=100;hooks_owned=False;owned=[];ids=[];motion=[];trace=[]
    def keep(g):owned.append(g);return g
    def snapshot():
        result=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g=obj.Geometry;row=dict(source=ids.index(obj.Id) if obj.Id in ids else None,selected=bool(obj.IsSelected(False)))
            if isinstance(g,G.Point):row.update(kind='point',point=host['_xyz'](g.Location))
            elif isinstance(g,G.Curve):
                n=g.ToNurbsCurve()
                try:row.update(kind='curve',definition=host['_nurbs_curve_definition'](n),samples=[host['_xyz'](g.PointAt(g.Domain.ParameterAt(i/32.))) for i in range(33)])
                finally:n.Dispose()
            else:
                s=g.Faces[0].UnderlyingSurface() if isinstance(g,G.Brep) else g;n=s.ToNurbsSurface()
                try:row.update(kind='surface',definition=host['_nurbs_surface_definition'](n))
                finally:n.Dispose()
            result.append(row)
        return result
    try:
        case=op['case'];closed=case.startswith('closed_');backward='backward' in case
        source=keep(G.PolylineCurve([G.Point3d(x,y,0.) for x,y in ((0.,0.),(4.,0.),(4.,6.),(0.,6.),(0.,0.))])) if closed else keep(G.LineCurve(G.Point3d(0.,0.,0.),G.Point3d(4.,6.,0.)))
        if case.startswith('closed_circle_'):source=keep(G.Circle(G.Plane.WorldXY,4.).ToNurbsCurve())
        ids.append(doc.Objects.AddCurve(source));doc.Objects.Select(ids[0]);anchor=source.Domain.ParameterAt(.2 if 'early' in case else (.8 if closed else .5));start=host['_xyz'](source.PointAt(anchor))
        aim=[4.,3.,0.] if closed and backward else ([3.,0.,0.] if closed else ([.8,1.2,0.] if backward else [3.2,4.8,0.]))
        if case.startswith('closed_circle_'):aim=host['_xyz'](source.PointAt(source.Domain.ParameterAt(.4 if 'early' in case else .1)))
        if case=='closed_early_backward_numeric':aim=[1.,0.,0.]
        endpoint=[3.,0.,0.] if closed and backward else ([4.,3.,0.] if closed else ([3.2,4.8,0.] if backward else [.8,1.2,0.]))
        inline=case.startswith('inline_');command='CreateUVCrv' if inline else 'SubCrv'
        if inline:
            surface=keep(G.NurbsSurface.CreateFromCorners(G.Point3d(0.,0.,0.),G.Point3d(4.,0.,0.),G.Point3d(4.,6.,0.),G.Point3d(0.,6.,0.)));ids.append(doc.Objects.AddSurface(surface));doc.Objects.UnselectAll()
        vp.SetProjection(Rhino.Display.DefinedViewportProjection.Top,'Owned SubCrv lock',False);vp.SetConstructionPlane(G.Plane.WorldXY);vp.ZoomBoundingBox(G.BoundingBox(G.Point3d(-1.,-1.,-1.),G.Point3d(5.,7.,1.)))
        pixel=vp.WorldToClient(host['_point'](aim));screen=view.ClientToScreen(System.Drawing.Point(int(pixel.X),int(pixel.Y)));marker='@subcurve-direction:'+op['id'];ready=os.path.join(os.path.dirname(host['__file__']),'subcurve-direction-ready-'+op['id']+'.json')
        locked=os.path.join(os.path.dirname(host['__file__']),'subcurve-direction-locked-'+op['id']+'.json')
        lock_prompts=[]
        free_prompts=[];parent_prompts=[];finish_prompts=[];completions=[];prompt_trace=[];typed_inputs=[];input_errors=[];sending=[False];last_input=[System.DateTime.UtcNow]
        def acknowledge(label):
            path=os.path.join(os.path.dirname(host['__file__']),'subcurve-direction-'+label+'-'+op['id']+'.json')
            with open(path+'.tmp','w') as f:json.dump(marker,f)
            os.rename(path+'.tmp',path)
        def tick(sender,event):
            if sending[0] or input_errors:return
            prompt=Rhino.RhinoApp.CommandPrompt
            if len(prompt_trace)<32 and (not prompt_trace or prompt_trace[-1]!=prompt):
                prompt_trace.append(prompt)
                with open(os.path.join(os.path.dirname(host['__file__']),'subcurve-direction-state-'+op['id']+'.json'),'w') as f:json.dump(prompt_trace,f)
            if motion and not lock_prompts and 'Direction=Locked' in prompt:
                lock_prompts.append(prompt)
                with open(locked+'.tmp','w') as f:json.dump(marker,f)
                os.rename(locked+'.tmp',locked)
            if lock_prompts and case=='unlock_point' and not free_prompts and 'Direction=Free' in prompt:
                free_prompts.append(prompt);acknowledge('free')
            if lock_prompts and inline and not parent_prompts and 'curve length' not in prompt and 'Select' in prompt:
                parent_prompts.append(prompt);acknowledge('parent')
            if lock_prompts and not inline and not completions and not finish_prompts and prompt.strip().rstrip(':')=='Select curve':
                finish_prompts.append(prompt);acknowledge('finish')
            if (System.DateTime.UtcNow-last_input[0]).TotalSeconds<.5:return
            ack=os.path.join(os.path.dirname(host['__file__']),'click-ack.json')
            if not os.path.exists(ack):return
            with open(ack) as f:
                if json.load(f)!=marker:return
            token=None
            if not typed_inputs:token='Direction=Locked' if case=='direction_option' else 'D'
            elif not lock_prompts:return
            elif case=='unlock_point' and len(typed_inputs)==1:token='Direction=Free'
            elif case=='unlock_point' and not free_prompts:return
            elif len(typed_inputs)==(2 if case=='unlock_point' else 1):
                if case.endswith('numeric'):token='8' if closed else '2'
                elif closed:token='3,0,0' if backward else '4,3,0'
                elif case=='same_side_point':token='3.2,4.8,0'
                else:token='3.2,4.8,0' if backward else '.8,1.2,0'
            elif (parent_prompts or finish_prompts) and typed_inputs[-1]!='':token=''
            if token is None:return
            typed_inputs.append(token);last_input[0]=System.DateTime.UtcNow;sending[0]=True
            try:Rhino.RhinoApp.SendKeystrokes(token,True)
            except Exception as error:input_errors.append(str(error));timer.Stop()
            finally:sending[0]=False
        def ended(sender,event):
            if event.CommandEnglishName==command:
                completions.append(str(event.CommandResult));acknowledge('complete')
        timer.Tick+=tick
        class Mouse(Rhino.UI.MouseCallback):
            def OnEndMouseMove(self,event):
                if len(trace)<12:trace.append(dict(pixel=[int(event.ViewportPoint.X),int(event.ViewportPoint.Y)],expected_pixel=[int(pixel.X),int(pixel.Y)],viewport=str(event.View.ActiveViewport.Id),expected_viewport=str(vp.Id),prompt=Rhino.RhinoApp.CommandPrompt))
                if motion or event.View.ActiveViewport.Id!=vp.Id or 'curve length' not in Rhino.RhinoApp.CommandPrompt:return
                if abs(int(event.ViewportPoint.X)-int(pixel.X))>1 or abs(int(event.ViewportPoint.Y)-int(pixel.Y))>1:return
                motion.append(dict(pixel=[int(event.ViewportPoint.X),int(event.ViewportPoint.Y)],prompt=Rhino.RhinoApp.CommandPrompt,aim=aim))
                with open(ready+'.tmp','w') as f:json.dump(marker,f)
                os.rename(ready+'.tmp',ready)
        macro=('_CreateUVCrv _SelID '+str(ids[1])+' _SubCrv _SelID '+str(ids[0]) if inline else '_SubCrv _Copy=_Yes _Mode=_'+('MarkEnds' if case.startswith('mark_') else 'Shorten')+' _FromMidpoint=_No')+' '+','.join(repr(x) for x in start)+' _Pause'
        doc.ClearUndoRecords(True);before=snapshot();history_marker='Viboceros direction '+str(System.Guid.NewGuid());Rhino.RhinoApp.WriteLine(history_marker);hooks_owned=True
        with environment(dict(persistent_snaps=[]),host):
            with _input_hooks(timer,Mouse(),[(Rhino.Commands.Command.EndCommand,ended)]):
                doc.Views.Redraw();host['_record_progress']('PICK %s %d %d'%(marker,screen.X,screen.Y))
                success,after,events=observe_command(Rhino.Commands.Command,command,lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        if input_errors:raise ValueError('direction input failed: '+str(input_errors))
        active=bool(Rhino.Commands.Command.InCommand())
        if active:Rhino.RhinoApp.RunScript('!',False)
        history=Rhino.RhinoApp.CommandHistoryWindowText.split(history_marker,1)[-1];Rhino.RhinoApp.RunScript('_Undo',False);undo=snapshot();Rhino.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(case=case,anchor=anchor,start=start,aim=aim,endpoint=endpoint,motion=motion,motion_trace=trace,prompt_trace=prompt_trace,lock_prompts=lock_prompts,free_prompts=free_prompts,parent_prompts=parent_prompts,finish_prompts=finish_prompts,completions=completions,typed_inputs=typed_inputs,macro=macro,before=before,after=after,success=success,command_active=active,history=history,events=events,undo=undo,redo=redo),0
    finally:
        if not hooks_owned:timer.Dispose()
        if Rhino.Commands.Command.InCommand():Rhino.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        vp.SetViewProjection(original,False);vp.SetConstructionPlane(plane);original.Dispose();doc.ModelAbsoluteTolerance=saved
        for g in reversed(owned):g.Dispose()
