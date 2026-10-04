"""Public ScaleByPlane commands in an empty owned document at idle."""
import re
import json
import os

if globals().get('__package__'):
    from .number_token import number_token
else:
    from number_token import number_token

PLANES = ('ActiveCPlane','WorldTop','WorldFront','WorldRight','3Point','Object','FromView')
SOURCES = ('point','rational','periodic','surface','mesh','line','circle','arc')
INPUTS = dict(ordinary=([2.,3.,0.],[4.,9.,0.]),negative_x=([2.,3.,0.],[-4.,9.,0.]),
              negative_y=([2.,3.,0.],[4.,-9.,0.]),offplane=([2.,3.,5.],[4.,9.,-2.]),
              zero_x=([2.,3.,0.],[0.,9.,0.]),zero_y=([2.,3.,0.],[4.,0.,0.]),
              zero_both=([2.,3.,0.],[0.,0.,0.]),reference_x_zero=([0.,3.,0.],[4.,9.,0.]),
              reference_y_zero=([2.,0.,0.],[4.,9.,0.]),reference_origin=([0.,0.,0.],[4.,9.,0.]))
REQUEST_INPUTS = tuple(INPUTS)
for label,value in (('below_1e_6',1e-6-1e-12),('above_1e_6',1e-6+1e-12),('2e_6',2e-6),('5e_6',5e-6),('1e_2',1e-2),('1e_3',1e-3),('1e_4',1e-4),('1e_5',1e-5),('1e_6',1e-6),('1e_7',1e-7),('sqrt_epsilon',2.**-26),('below_sqrt_epsilon',(2.**-26)*(1.-2.**-26)),('above_sqrt_epsilon',(2.**-26)*(1.+2.**-26)),('1e_8',1e-8),('1e_10',1e-10),('1e_12',1e-12),('1e_14',1e-14)):
    INPUTS['tiny_reference_'+label] = ([value,3.,0.],[4.,9.,0.])
    INPUTS['tiny_target_'+label] = ([2.,3.,0.],[value,9.,0.])

for label,value in (('below',1e-6-2.**-73),('exact',1e-6),('above',1e-6+2.**-73),('negative',-1e-6)):
    INPUTS['exact_reference_'+label] = ([value,3.,0.],[4.,9.,0.])
    INPUTS['exact_target_'+label] = ([2.,3.,0.],[value,9.,0.])

def boundary_request():
    base = request()['operations'][1]
    rows = [dict(base,id='scale-by-plane-small-'+str(i),input=k) for i,k in enumerate(INPUTS) if k not in REQUEST_INPUTS]
    for i,view in enumerate(('Top','Front','Perspective')):
        for cplane in ('world','tilted'):
            rows.append(dict(base,id='scale-by-plane-view-'+str(i)+'-'+cplane,plane='FromView',cplane=cplane,view=view))
    return dict(protocol_version=1,iterations=1,operations=rows)

def validate_request(q):
    if (not isinstance(q,dict) or type(q.get('protocol_version')) is not int or q['protocol_version']!=1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1)!=1
            or not isinstance(q.get('operations'),list) or not 1 <= len(q['operations']) <= 64):
        raise ValueError('ScaleByPlane requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op)!={'op','id','source','plane','cplane','view','input','copy','rigid','selection'}
                or op['op']!='scale_by_plane' or not isinstance(op['id'],str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None or op['id'] in seen
                or op['source'] not in SOURCES or op['plane'] not in PLANES
                or op['cplane'] not in ('world','tilted') or op['view'] not in ('Top','Front','Perspective')
                or op['input'] not in tuple(INPUTS) or type(op['copy']) is not bool or type(op['rigid']) is not bool
                or op['selection'] not in ('objects','grips','parent','post')
                or op['selection'] in ('grips','parent') and op['source'] not in ('rational','periodic','surface','mesh')):
            raise ValueError('invalid ScaleByPlane recipe')
        seen.add(op['id'])


def request():
    rows = [('point',plane,'ordinary',False,False,'objects','Top') for plane in PLANES]
    rows += [('point','FromView','ordinary',False,False,'objects',view) for view in ('Front','Perspective')]
    rows += [('point','WorldTop',kind,False,False,'objects','Top') for kind in REQUEST_INPUTS if kind!='ordinary']
    rows += [(source,'ActiveCPlane','ordinary',copy,rigid,'objects','Top')
             for source in SOURCES if source!='point' for copy,rigid in ((False,False),(True,False),(False,True),(True,True))]
    rows += [(source,'3Point','ordinary',copy,False,'grips','Top')
             for source in ('rational','surface','mesh') for copy in (False,True)]
    rows += [(source,'WorldFront','ordinary',False,True,'parent','Top') for source in ('rational','surface','mesh')]
    rows += [('rational',plane,'ordinary',True,False,'post','Top') for plane in ('Object','WorldRight','FromView')]
    return dict(protocol_version=1,iterations=1,operations=[dict(op='scale_by_plane',id='scale-by-plane-'+str(i),
        source=s,plane=p,cplane='tilted',view=v,input=t,copy=c,rigid=r,selection=k)
        for i,(s,p,t,c,r,k,v) in enumerate(rows)])


def token(point):
    return 'w'+','.join(number_token(float(x)) for x in point)


def run(op,host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino,System = host['Rhino'],host['System']
    doc = Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand(): raise ValueError('ScaleByPlane requires idle execution')
    if list(doc.Objects): raise ValueError('ScaleByPlane requires an empty owned document')
    from grip_transform_probe import create_source
    from join_probe import observe_command
    import clr
    clr.AddReference('System.Windows.Forms')
    from System.Windows.Forms import Timer
    from shrink_face_input import _input_hooks
    vp = doc.Views.ActiveView.ActiveViewport
    old_view = Rhino.DocObjects.ViewportInfo(vp); old_plane = vp.GetConstructionPlane()
    ids,target_id = [],None
    pending,errors,picked = [],[],[]
    timer = Timer(); timer.Interval = 100
    marker = '@scale-by-plane-view:'+op['id']
    root = os.path.dirname(os.path.abspath(host['__file__']))
    ready = os.path.join(root,'scale-by-plane-view-ready-'+op['id']+'.json')
    started = System.DateTime.UtcNow
    def tick(sender,event):
        if not errors and (System.DateTime.UtcNow-started).TotalSeconds>30:
            errors.append('ScaleByPlane viewport pick timed out')
            host['_record_progress']('PICK_ABORT '+op['id']); timer.Stop()
    class Mouse(Rhino.UI.MouseCallback):
        def OnEndMouseMove(self,event):
            if (pending or errors or event.View.ActiveViewport.Id!=vp.Id
                    or [int(event.ViewportPoint.X),int(event.ViewportPoint.Y)]!=[x,y]
                    or not Rhino.RhinoApp.CommandPrompt.startswith('Select Viewport')): return
            pending.append(dict(prompt=Rhino.RhinoApp.CommandPrompt,view=str(vp.Id)))
            with open(ready+'.tmp','w') as stream: json.dump(marker,stream)
            os.rename(ready+'.tmp',ready)
        def OnMouseDown(self,event):
            if Rhino.RhinoApp.CommandPrompt.startswith('Select Viewport'):
                picked.append(dict(view=str(event.View.ActiveViewport.Id),
                                   construction_plane=frame(event.View.ActiveViewport.GetConstructionPlane().Plane)))
    def frame(plane):
        return dict(origin=host['_xyz'](plane.Origin),x_axis=host['_xyz'](plane.XAxis),y_axis=host['_xyz'](plane.YAxis))
    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects),key=lambda obj:obj.RuntimeSerialNumber):
            if obj.Id==target_id: continue
            geometry = obj.Geometry
            row = dict(role='source' if obj.Id in ids else 'output',kind=geometry.GetType().Name,name=obj.Attributes.Name,
                       selected=bool(obj.IsSelected(False)),grips_on=bool(obj.GripsOn),
                       grips=[dict(index=int(g.Index),point=host['_xyz'](g.CurrentLocation),selected=bool(g.IsSelected(False)))
                              for g in (obj.GetGrips() or [])])
            if isinstance(geometry,Rhino.Geometry.Point): row['point'] = host['_xyz'](geometry.Location)
            elif isinstance(geometry,Rhino.Geometry.Curve): row['curve'] = host['_nurbs_curve_definition'](geometry.ToNurbsCurve())
            elif isinstance(geometry,Rhino.Geometry.Brep): row['surface'] = host['_nurbs_surface_definition'](geometry.Surfaces[0].ToNurbsSurface())
            elif isinstance(geometry,Rhino.Geometry.Mesh): row['mesh'] = host['_polygon_mesh_value'](geometry)
            else: raise ValueError('unexpected ScaleByPlane geometry')
            box = geometry.GetBoundingBox(True)
            row['center'] = host['_xyz'](box.Center)
            rows.append(row)
        return rows
    try:
        if not vp.SetProjection(getattr(Rhino.Display.DefinedViewportProjection,op['view']),'Owned ScaleByPlane',False):
            raise ValueError('ScaleByPlane view setup failed')
        active = Rhino.Geometry.Plane.WorldXY if op['cplane']=='world' else Rhino.Geometry.Plane(
            host['_point']([5.,-4.,2.]),Rhino.Geometry.Vector3d(1.,1.,0.),Rhino.Geometry.Vector3d(-1.,1.,2.))
        vp.SetConstructionPlane(active)
        ok,view_plane = vp.GetCameraFrame()
        if not ok: raise ValueError('ScaleByPlane camera frame unavailable')
        chosen = active
        if op['plane']=='WorldTop': chosen = Rhino.Geometry.Plane.WorldXY
        elif op['plane']=='WorldFront': chosen = Rhino.Geometry.Plane(host['_point']([0.,0.,0.]),Rhino.Geometry.Vector3d.XAxis,Rhino.Geometry.Vector3d.ZAxis)
        elif op['plane']=='WorldRight': chosen = Rhino.Geometry.Plane.WorldYZ
        elif op['plane']=='FromView': chosen = view_plane
        serial = doc.BeginUndoRecord('ScaleByPlane sources')
        try:
            attributes = Rhino.DocObjects.ObjectAttributes(); attributes.Name = 'plane-scale source'
            geometry = Rhino.Geometry.Point(host['_point']([2.,3.,4.])) if op['source']=='point' else create_source(op['source'],host)
            try: source_id = doc.Objects.Add(geometry,attributes)
            finally: geometry.Dispose()
            if source_id==System.Guid.Empty: raise ValueError('ScaleByPlane source insertion failed')
            ids.append(source_id)
            if op['plane']=='Object':
                geometry = Rhino.Geometry.PlaneSurface(chosen,Rhino.Geometry.Interval(-2.,2.),Rhino.Geometry.Interval(-2.,2.))
                try: target_id = doc.Objects.AddSurface(geometry)
                finally: geometry.Dispose()
                if target_id==System.Guid.Empty: raise ValueError('ScaleByPlane target insertion failed')
        finally: doc.EndUndoRecord(serial)
        obj = doc.Objects.FindId(source_id)
        if op['selection'] in ('grips','parent'):
            obj.GripsOn = True
            for index in (0,2): obj.GetGrips()[index].Select(True)
        if op['selection'] in ('objects','parent'): doc.Objects.Select(source_id)
        origin = host['_point']([0.,0.,0.] if op['input'].startswith('exact_') else [1.,2.,3.])
        def world(offset):
            return host['_xyz'](origin+chosen.XAxis*offset[0]+chosen.YAxis*offset[1]+chosen.ZAxis*offset[2])
        reference,target = [world(p) for p in INPUTS[op['input']]]
        plane_text = '_Plane=_'+op['plane']
        if op['plane']=='3Point':
            plane_text += ' '+token(host['_xyz'](chosen.Origin))+' '+token(host['_xyz'](chosen.Origin+chosen.XAxis))+' '+token(host['_xyz'](chosen.Origin+chosen.YAxis))
        elif op['plane']=='Object': plane_text += ' _SelID '+str(target_id)
        elif op['plane']=='FromView': plane_text += ' _Pause'
        macro = '_ScaleByPlane '+('_SelID '+str(source_id)+' _Enter ' if op['selection']=='post' else '')
        macro += '_Copy=_'+('Yes' if op['copy'] else 'No')+' _Rigid=_'+('Yes' if op['rigid'] else 'No')+' '+plane_text
        macro += ' '+token(host['_xyz'](origin))+' '+token(reference)+' '+token(target)
        macro += (' _Enter' if op['copy'] else '')+' _Cancel'
        before = snapshot(); history_marker = 'Viboceros ScaleByPlane '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(history_marker)
        host['_record_progress'](op['id']+' '+macro)
        if op['plane']=='FromView':
            x,y = int(vp.Size.Width//2),int(vp.Size.Height//2)
            screen = doc.Views.ActiveView.ClientToScreen(System.Drawing.Point(x,y))
            with _input_hooks(timer,Mouse(),[(timer.Tick,tick)]):
                host['_record_progress']('PICK '+marker+' %d %d' % (screen.X,screen.Y))
                success,after,events = observe_command(Rhino.Commands.Command,'ScaleByPlane',
                    lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
            if errors or len(pending)!=1: raise ValueError('ScaleByPlane viewport input incomplete: '+str(errors))
        else:
            timer.Dispose()
            success,after,events = observe_command(Rhino.Commands.Command,'ScaleByPlane',
                lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        if Rhino.Commands.Command.InCommand(): raise ValueError('ScaleByPlane macro remains active')
        after_script = snapshot(); history = Rhino.RhinoApp.CommandHistoryWindowText.split(history_marker,1)[-1]
        Rhino.RhinoApp.RunScript('_Undo',False); undo = snapshot()
        Rhino.RhinoApp.RunScript('_Redo',False); redo = snapshot()
        return dict(active_plane=frame(active),plane=frame(chosen),view_plane=frame(view_plane),
                    origin=host['_xyz'](origin),reference=reference,target=target,macro=macro,
                    before=before,after=after,after_script=after_script,undo=undo,redo=redo,
                    history=history,events=events,success=success,view_pick=pending,view_clicked=picked),0
    finally:
        timer.Dispose()
        for obj in list(doc.Objects):
            if obj.GripsOn: obj.GripsOn = False
            doc.Objects.Delete(obj.Id,True)
        vp.SetViewProjection(old_view,False); vp.SetConstructionPlane(old_plane)
        old_view.Dispose(); doc.Views.Redraw()
