"""Owned ScalePositions point input with acknowledged native mouse motion."""
import json
import os
import re


def validate_request(request):
    if (not isinstance(request,dict) or type(request.get('protocol_version')) is not int
            or request['protocol_version'] != 1 or type(request.get('iterations',1)) is not int
            or request.get('iterations',1) != 1 or not isinstance(request.get('operations'),list)
            or not 1 <= len(request['operations']) <= 64):
        raise ValueError('ScalePositions cursor capture requires bounded protocol 1 recipes')
    seen = set()
    for op in request['operations']:
        if (not isinstance(op,dict) or set(op) != {'op','id','input','mode','view','aim','finish','copy','source','origin','factor'}
                or op['op'] != 'scale_positions_cursor' or not isinstance(op['id'],str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None or op['id'] in seen
                or op['input'] not in ('Numeric','Default','Reference')
                or op['mode'] not in ('1d','2d','3d')
                or op['input'] != 'Reference' and op['mode'] != '1d'
                or op['view'] not in ('Top','Front','Perspective')
                or op['aim'] not in ('Near','Far','OffAxis','Vertical')
                or op['view']=='Top' and op['aim']=='Vertical'
                or op['finish'] not in ('Click','Typed','ClickCancel') or type(op['copy']) is not bool
                or op['source'] not in ('point','line','rational')
                or op['origin'] not in ('Zero','Offset','X','Y','Z','Tiny','Near','Negative','Below','At','Above',
                                        'AboveNext','Corner','Diagonal','NegativeTiny','NegativeAt','E9','E8','E7','E6','E5','E4',
                                        'SqrtBelow','SqrtAt','SqrtAbove','SqrtCorner','FloatBelow','FloatAt','FloatAbove','FloatCorner')
                or type(op['factor']) not in (float,int) or op['factor'] not in (.5,2.,3.)
                or op['input']=='Reference' and op['factor']!=2.
                or op['finish']=='ClickCancel' and (op['input']=='Reference' or op['view']!='Front' or op['copy'])):
            raise ValueError('invalid ScalePositions cursor recipe')
        seen.add(op['id'])


def recipe(op):
    try:
        from .number_token import number_token
    except (ImportError,ValueError):
        from number_token import number_token
    aim = dict(Near=[4.,0.,0.],Far=[8.,0.,0.],OffAxis=[4.,5.,0.],Vertical=[4.,0.,5.])[op['aim']]
    first = dict(Numeric=str(op['factor']),Default='_Enter',Reference='w2,0,0')[op['input']]
    origin = dict(Zero=[0.,0.,0.],Offset=[1.,2.,3.],X=[1.,0.,0.],Y=[0.,2.,0.],Z=[0.,0.,3.],
                  Tiny=[1e-10,0.,0.],Near=[.001,0.,0.],Negative=[-1.,-2.,-3.],Below=[2.**-33,0.,0.],
                  At=[2.**-32,0.,0.],Above=[2.**-31,0.,0.],AboveNext=[2.3283064365386968e-10,0.,0.],
                  Corner=[2e-10,2e-10,2e-10],Diagonal=[1e-10,1e-10,1e-10],
                  NegativeTiny=[-1e-10,0.,0.],NegativeAt=[-2.**-32,0.,0.],
                  E9=[1e-9,0.,0.],E8=[1e-8,0.,0.],E7=[1e-7,0.,0.],E6=[1e-6,0.,0.],
                  E5=[1e-5,0.,0.],E4=[1e-4,0.,0.],SqrtBelow=[1.4901161193847655e-8,0.,0.],
                  SqrtAt=[2.**-26,0.,0.],SqrtAbove=[1.490116119384766e-8,0.,0.],
                  SqrtCorner=[1e-8,1e-8,1e-8],FloatBelow=[1.1920928955078124e-7,0.,0.],
                  FloatAt=[2.**-23,0.,0.],FloatAbove=[1.1920928955078128e-7,0.,0.],
                  FloatCorner=[1e-7,1e-7,1e-7])[op['origin']]
    macro = '_ScalePositions _Copy=_'+('Yes' if op['copy'] else 'No')+' w'+','.join(str(x) if op['origin'] in ('Zero','Offset') else number_token(x) for x in origin)+' '+first+' _Pause'
    if op['copy']: macro += ' _Enter'
    return dict(origin=origin,reference=[2.,0.,0.] if op['input']=='Reference' else None,
                factor=float(op['factor']),aim=aim,typed_target='w6,3,2',macro=macro)


def request():
    rows = [(kind,'1d',view,aim,finish,False)
            for kind in ('Numeric','Default','Reference')
            for view,aim in (('Top','Near'),('Top','OffAxis'),('Front','Vertical'),('Perspective','Far'))
            for finish in ('Click','Typed')]
    rows += [('Reference',mode,'Top',aim,finish,False)
             for mode in ('2d','3d') for aim in ('Near','OffAxis') for finish in ('Click','Typed')]
    rows += [(kind,'1d','Top','OffAxis',finish,True)
             for kind in ('Numeric','Default','Reference') for finish in ('Click','Typed')]
    rows = [row+('point','Zero') for row in rows]
    rows += [(kind,'1d','Top','OffAxis',finish,False,source,'Offset')
             for kind in ('Numeric','Default') for source in ('point','line','rational') for finish in ('Click','Typed')]
    rows += [(kind,'1d','Top','OffAxis',finish,False,source,'Zero')
             for kind in ('Numeric','Default') for source in ('line','rational') for finish in ('Click','Typed')]
    rows += [('Reference',mode,'Top','OffAxis',finish,False,'point','Offset')
             for mode in ('1d','2d','3d') for finish in ('Click','Typed')]
    return dict(protocol_version=1,iterations=1,operations=[
        dict(op='scale_positions_cursor',id='scale-positions-cursor-'+str(i),input=kind,mode=mode,
             view=view,aim=aim,finish='ClickCancel' if view=='Front' and kind!='Reference' and finish=='Click' else finish,
             copy=copy_option,source=source,origin=origin,factor=2.)
        for i,(kind,mode,view,aim,finish,copy_option,source,origin) in enumerate(rows)])


def origin_request():
    base = request()['operations'][3]
    rows = [dict(base,origin=origin,factor=factor,finish='Typed')
            for origin in ('Zero','Offset','X','Y','Z','Tiny','Near','Negative') for factor in (2.,3.)]
    rows += [dict(base,origin='Offset',source=source,factor=.5,input='Default',finish=finish)
             for source in ('point','rational') for finish in ('Click','Typed')]
    rows += [dict(base,origin=origin,factor=3.,finish='Typed',copy=copy_option)
             for origin in ('Below','At','Above','AboveNext','Corner','Diagonal','NegativeTiny','NegativeAt','E9','E8','E7','E6','E5','E4',
                                        'SqrtBelow','SqrtAt','SqrtAbove','SqrtCorner','FloatBelow','FloatAt','FloatAbove','FloatCorner')
             for copy_option in (False,True)]
    for i,op in enumerate(rows): op['id'] = 'scale-positions-origin-'+str(i)
    return dict(protocol_version=1,iterations=1,operations=rows)


def run(op,host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino,System = host['Rhino'],host['System']
    doc = Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand(): raise ValueError('ScalePositions cursor requires idle execution')
    if list(doc.Objects): raise ValueError('ScalePositions cursor requires an empty owned document')
    import clr
    clr.AddReference('System.Windows.Forms')
    from System.Windows.Forms import Timer
    from shrink_face_input import _input_hooks
    from snap_environment import environment
    from viewport_capture import capture
    from named_view_policy_probe import snapshot as camera_snapshot
    from join_probe import observe_command
    from grip_transform_probe import create_source
    view = doc.Views.ActiveView
    vp = view.ActiveViewport
    original = Rhino.DocObjects.ViewportInfo(vp)
    original_plane = vp.GetConstructionPlane()
    root = os.path.dirname(os.path.abspath(host['__file__']))
    marker = '@scale-positions-cursor:'+op['id']
    ready = os.path.join(root,'scale-positions-cursor-ready-'+op['id']+'.json')
    rejected = os.path.join(root,'scale-positions-cursor-rejected-'+op['id']+'.json')
    pending,errors,active = [],[],[]
    timer = Timer(); timer.Interval = 100
    hooks_owned = False
    started = System.DateTime.UtcNow
    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects),key=lambda obj:obj.RuntimeSerialNumber):
            geometry = obj.Geometry
            row = dict(selected=bool(obj.IsSelected(False)))
            if isinstance(geometry,Rhino.Geometry.Point): row['point'] = host['_xyz'](geometry.Location)
            elif isinstance(geometry,Rhino.Geometry.Curve): row['curve'] = host['_nurbs_curve_definition'](geometry.ToNurbsCurve())
            else: raise ValueError('unexpected ScalePositions cursor source')
            rows.append(row)
        return rows
    def begun(sender,event):
        if event.CommandEnglishName=='ScalePositions': active.append(True)
    def ended(sender,event):
        if event.CommandEnglishName=='ScalePositions':
            active[:] = []
            host['_record_progress']('CURSOR_END '+op['id'])
    def tick(sender,event):
        if not errors and (System.DateTime.UtcNow-started).TotalSeconds > 30:
            errors.append('ScalePositions cursor timed out')
            host['_record_progress']('PICK_ABORT '+op['id']); timer.Stop()
    class Mouse(Rhino.UI.MouseCallback):
        def OnEndMouseDown(self,event):
            if (op['finish']=='ClickCancel' and active and pending and not errors
                    and event.View.ActiveViewport.Id==vp.Id and Rhino.Input.RhinoGet.InGetPoint(doc)):
                pending[0]['rejected_click'] = dict(prompt=Rhino.RhinoApp.CommandPrompt,objects=snapshot())
                with open(rejected+'.tmp','w') as stream: json.dump(marker,stream)
                os.rename(rejected+'.tmp',rejected)
        def OnEndMouseMove(self,event):
            if (not active or pending or errors or event.View.ActiveViewport.Id != vp.Id
                    or [int(event.ViewportPoint.X),int(event.ViewportPoint.Y)] != [x,y]
                    or not Rhino.Input.RhinoGet.InGetPoint(doc)): return
            try:
                ok,ray = vp.GetFrustumLine(x,y)
                if not ok: raise ValueError('ScalePositions cursor viewing line unavailable')
                frame = capture(vp,host['_point'](spec['aim']),[x,y],host)
                frame['ray'] = [host['_xyz'](ray.From),host['_xyz'](ray.To)]
                pending.append(dict(frame=frame,camera=camera_snapshot(vp,Rhino),objects=snapshot(),
                                    prompt=Rhino.RhinoApp.CommandPrompt,
                                    phase='second_reference' if op['input']=='Reference' else 'direction'))
                with open(ready+'.tmp','w') as stream: json.dump(marker,stream)
                os.rename(ready+'.tmp',ready)
                host['_record_progress']('CURSOR_READY '+op['id'])
            except Exception as error:
                errors.append(str(error)); host['_record_progress']('PICK_ABORT '+op['id'])
    try:
        if not vp.SetProjection(getattr(Rhino.Display.DefinedViewportProjection,op['view']),'Owned ScalePositions cursor',False):
            raise ValueError('ScalePositions cursor projection failed')
        vp.SetConstructionPlane(Rhino.Geometry.Plane.WorldXY)
        if not vp.ZoomBoundingBox(Rhino.Geometry.BoundingBox(host['_point']([-12.,-10.,-10.]),host['_point']([12.,10.,10.]))):
            raise ValueError('ScalePositions cursor zoom failed')
        seed = doc.Objects.AddPoint(host['_point']([1.,1.,1.])); doc.Objects.Select(seed)
        seed_macro = '_ScalePositions _Copy=_No _Mode=_'+op['mode'].upper()+' w0,0,0 w1,0,0 w2,0,0'
        if op['input']=='Default' and op['factor']!=2.:
            seed_macro = '_ScalePositions _Copy=_No _Mode=_'+op['mode'].upper()+' w0,0,0 w2,0,0 w'+str(2.*op['factor'])+',0,0'
        if not Rhino.RhinoApp.RunScript(seed_macro,True) or Rhino.Commands.Command.InCommand():
            raise ValueError('ScalePositions cursor seed failed')
        for obj in list(doc.Objects): doc.Objects.Delete(obj.Id,True)
        serial = doc.BeginUndoRecord('ScalePositions cursor source')
        try:
            if op['source']=='point': source = doc.Objects.AddPoint(host['_point']([2.,3.,4.]))
            else:
                geometry = create_source(op['source'],host)
                try: source = doc.Objects.AddCurve(geometry)
                finally: geometry.Dispose()
        finally: doc.EndUndoRecord(serial)
        if source == System.Guid.Empty or not doc.Objects.Select(source):
            raise ValueError('ScalePositions cursor source insertion/selection failed')
        geometry = doc.Objects.FindId(source).Geometry
        box = geometry.GetBoundingBox(True)
        bounds = dict(min=host['_xyz'](box.Min),max=host['_xyz'](box.Max),center=host['_xyz'](box.Center))
        before = snapshot(); spec = recipe(op)
        pixel = vp.WorldToClient(host['_point'](spec['aim'])); x,y = int(pixel.X),int(pixel.Y)
        if not 1 <= x < vp.Size.Width-1 or not 1 <= y < vp.Size.Height-1:
            raise ValueError('ScalePositions cursor outside viewport')
        screen = view.ClientToScreen(System.Drawing.Point(x,y))
        history_marker = 'Viboceros ScalePositions cursor '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(history_marker)
        with environment(dict(persistent_snaps=[]),host):
            hooks_owned = True
            with _input_hooks(timer,Mouse(),[(Rhino.Commands.Command.BeginCommand,begun),
                    (Rhino.Commands.Command.EndCommand,ended),(timer.Tick,tick)]):
                doc.Views.Redraw()
                host['_record_progress']('PICK '+marker+' %d %d' % (screen.X,screen.Y))
                success,after,events = observe_command(Rhino.Commands.Command,'ScalePositions',
                    lambda:Rhino.RhinoApp.RunScript(spec['macro'],True),snapshot,lambda:[],True)
        if errors or len(pending)!=1: raise ValueError('ScalePositions cursor incomplete: '+str(errors))
        after_script = snapshot()
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(history_marker,1)[-1]
        Rhino.RhinoApp.RunScript('_Undo',False); undo = snapshot()
        Rhino.RhinoApp.RunScript('_Redo',False); redo = snapshot()
        doc.Objects.UnselectAll(); doc.Objects.Select(source)
        preference_marker = 'Viboceros ScalePositions cursor defaults '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(preference_marker)
        preference_macro = '_ScalePositions w0,0,0 _Cancel'
        Rhino.RhinoApp.RunScript(preference_macro,True)
        preference_history = Rhino.RhinoApp.CommandHistoryWindowText.split(preference_marker,1)[-1]
        choice = re.search(r'Origin point \( Copy=(Yes|No)  Mode=(1D|2D|3D) \)',preference_history)
        scalar = re.search(r'Scale factor or first reference point <([^>]+)>',preference_history)
        if choice is None or scalar is None or Rhino.Commands.Command.InCommand():
            raise ValueError('ScalePositions cursor default query failed: '+preference_history)
        return dict(before=before,pending=pending[0],after=after,after_script=after_script,undo=undo,redo=redo,
                    events=events,success=success,history=history,recipe=spec,seed_macro=seed_macro,bounds=bounds,
                    preference_history=preference_history,preference_macro=preference_macro,
                    preferences=dict(mode=choice.group(2),factor=float(scalar.group(1)),copy=choice.group(1)=='Yes')),0
    finally:
        if not hooks_owned: timer.Dispose()
        for obj in list(doc.Objects): doc.Objects.Delete(obj.Id,True)
        vp.SetViewProjection(original,False); vp.SetConstructionPlane(original_plane)
        original.Dispose(); doc.Views.Redraw()
