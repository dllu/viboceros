"""Owned native Move/Copy cursor previews with public camera calibration."""
import json
import os
import re


def validate_request(request):
    if not isinstance(request,dict):
        raise ValueError('translation preview requires a request object')
    operations=request.get('operations')
    if (type(request.get('protocol_version')) is not int or request['protocol_version'] != 1
            or type(request.get('iterations',1)) is not int or request.get('iterations',1) != 1
            or not isinstance(operations,list) or not 1 <= len(operations) <= 48):
        raise ValueError('translation preview requires bounded protocol 1 cases')
    names=set()
    for op in operations:
        if (not isinstance(op,dict) or set(op) != {'op','id','command','placement','view','display_mode','finish'}
                or op['op'] != 'translation_preview' or not isinstance(op['id'],str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None or op['id'] in names
                or op['command'] not in ('Move','Copy') or op['view'] not in ('Top','Front','Perspective')
                or op['display_mode'] not in ('Wireframe','Shaded','Ghosted') or op['finish'] not in ('Click','Cancel')
                or op['placement'] not in ('Free','Vertical','Repeat','FromLastPoint','UseLastDistance','UseLastDirection','Normal')
                or op['command'] == 'Move' and op['placement'] in ('Repeat','FromLastPoint','UseLastDistance','UseLastDirection')
                or op['command'] == 'Copy' and op['placement'] == 'Normal'
                or op['placement'] == 'Vertical' and op['view'] != 'Front'):
            raise ValueError('invalid translation preview case')
        names.add(op['id'])


def request():
    cases=[]
    def add(command,placement,view,mode,finish='Click'):
        cases.append(dict(op='translation_preview',id='_'.join((command,placement,view,mode,finish)),
            command=command,placement=placement,view=view,display_mode=mode,finish=finish))
    for mode in ('Wireframe','Shaded','Ghosted'):
        for command in ('Move','Copy'):
            for view in ('Top','Perspective'): add(command,'Free',view,mode)
            add(command,'Vertical','Front',mode)
        for placement in ('Repeat','FromLastPoint','UseLastDistance','UseLastDirection'):
            add('Copy',placement,'Top',mode)
        add('Move','Normal','Top',mode)
    for command in ('Move','Copy'):
        add(command,'Free','Top','Shaded','Cancel')
    add('Copy','Repeat','Top','Ghosted','Cancel')
    return dict(protocol_version=1,iterations=1,operations=cases)


def run(operation,host):
    validate_request(dict(protocol_version=1,operations=[operation]))
    Rhino,System=host['Rhino'],host['System']; document=Rhino.RhinoDoc.ActiveDoc
    if list(document.Objects): raise ValueError('translation preview requires an empty owned document')
    import clr
    clr.AddReference('System.Windows.Forms')
    from System.Windows.Forms import Timer
    from shrink_face_input import _input_hooks
    from snap_environment import environment
    from viewport_capture import capture
    from named_view_policy_probe import snapshot as camera_snapshot
    view=document.Views.ActiveView; viewport=view.ActiveViewport
    original=Rhino.DocObjects.ViewportInfo(viewport)
    name,mode,target=viewport.Name,viewport.DisplayMode,viewport.CameraTarget
    root=os.path.dirname(os.path.abspath(host['__file__']))
    progress=os.path.join(root,'worker-progress.log')
    captured=os.path.join(root,'translation-preview-captured-'+operation['id']+'.json')
    ready=os.path.join(root,'translation-preview-ready-'+operation['id']+'.json')
    pending,errors,sources,reference,active,reference_click=[],[],[],[],[],[]
    timer=Timer(); timer.Interval=100; hooks_owned=False
    def snapshot():
        return [dict(source=sources.index(obj.Id) if obj.Id in sources else None,
            reference=obj.Id in reference,selected=bool(obj.IsSelected(False)),type=str(obj.Geometry.ObjectType),
            vertices=[host['_xyz'](vertex.Location) for vertex in obj.Geometry.Vertices]
                if isinstance(obj.Geometry,Rhino.Geometry.Brep) else None,
            bounds=[host['_xyz'](obj.Geometry.GetBoundingBox(True).Min),host['_xyz'](obj.Geometry.GetBoundingBox(True).Max)])
            for obj in document.Objects]
    def send(line):
        with open(progress,'a') as stream: stream.write(line+'\n'); stream.flush()
    def begun(sender,event):
        if event.CommandEnglishName==operation['command']:
            active.append(True);send('PREVIEW_BEGIN '+event.CommandEnglishName)
    def ended(sender,event):
        if event.CommandEnglishName==operation['command']:
            active[:]=[];send('PREVIEW_END '+event.CommandEnglishName)
    def observe():
        if pending or errors:return
        try:
            if not active or not Rhino.Input.RhinoGet.InGetPoint(document): raise ValueError('translation preview outside active GetPoint')
            prompt=Rhino.RhinoApp.CommandPrompt
            ok,ray=viewport.GetFrustumLine(x,y)
            if not ok: raise ValueError('translation preview ray unavailable')
            frame=capture(viewport,host['_point'](aim),[x,y],host)
            frame['ray']=[host['_xyz'](ray.From),host['_xyz'](ray.To)]
            pending.append(dict(objects=snapshot(),frame=frame,camera=camera_snapshot(viewport,Rhino),prompt=prompt))
            timer.Stop()
            with open(ready+'.tmp','w') as stream: json.dump(operation['id'],stream)
            os.rename(ready+'.tmp',ready)
        except Exception as error:
            errors.append(str(error));send('PICK_ABORT translation-preview-'+operation['id'])
    def tick(sender,event):
        if not pending and not errors and os.path.exists(captured):
            with open(captured) as stream:
                if json.load(stream)!=operation['id']:raise ValueError('foreign translation capture acknowledgement')
            observe()
    class Mouse(Rhino.UI.MouseCallback):
        def OnMouseDown(self,event):
            if operation['placement']!='Normal' or reference_click or pending:return
            try:
                if (not active or event.View.ActiveViewport.Id!=viewport.Id
                        or not Rhino.Input.RhinoGet.InGetObject(document)
                        or Rhino.Input.RhinoGet.InGetPoint(document)):
                    raise ValueError('Move Normal preview reference click outside owned GetObject')
                clicked=[int(event.ViewportPoint.X),int(event.ViewportPoint.Y)]
                if clicked!=reference_pixel:raise ValueError('unexpected Move Normal preview reference location')
                reference_click.append(clicked)
                send('PREVIEW_REFERENCE_CLICK '+str(clicked))
            except Exception as error:
                errors.append(str(error));send('PICK_ABORT translation-preview-'+operation['id'])
        def OnEndMouseMove(self,event):
            if (active and not pending and not errors and event.View.ActiveViewport.Id==viewport.Id
                    and [int(event.ViewportPoint.X),int(event.ViewportPoint.Y)]==[x,y]
                    and Rhino.Input.RhinoGet.InGetPoint(document)
                    and (operation['placement']!='Normal' or Rhino.RhinoApp.CommandPrompt.startswith('Point to move normal to'))):
                observe()
    try:
        projection=getattr(Rhino.Display.DefinedViewportProjection,operation['view'])
        if not viewport.SetProjection(projection,'Owned translation preview',False): raise ValueError('translation projection failed')
        viewport.SetConstructionPlane(Rhino.Geometry.Plane.WorldXY)
        viewport.DisplayMode=Rhino.Display.DisplayModeDescription.FindByName(operation['display_mode'])
        if not viewport.ZoomBoundingBox(Rhino.Geometry.BoundingBox(host['_point']([-14,-7,-1]),host['_point']([8,8,8]))):
            raise ValueError('translation preview zoom failed')
        attrs=Rhino.DocObjects.ObjectAttributes();attrs.ColorSource=Rhino.DocObjects.ObjectColorSource.ColorFromObject
        attrs.ObjectColor=System.Drawing.Color.FromArgb(200,80,60)
        sources.append(document.Objects.AddPoint(host['_point']([3,3,0]),attrs))
        curve=Rhino.Geometry.PolylineCurve([host['_point'](p) for p in ([2,1,0],[4,1,0],[3,-1,0],[2,1,0])])
        try: sources.append(document.Objects.AddCurve(curve,attrs))
        finally: curve.Dispose()
        box=Rhino.Geometry.Brep.CreateFromBox(Rhino.Geometry.BoundingBox(host['_point']([2,-5,0]),host['_point']([4,-3,2])))
        try: sources.append(document.Objects.AddBrep(box,attrs))
        finally: box.Dispose()
        if any(key == System.Guid.Empty for key in sources): raise ValueError('translation preview source insertion failed')
        placement=operation['placement']; base=[0,0,0]; aim=[-6,2,0]
        prefix='_'+operation['command']+' '
        if placement == 'Vertical': prefix+='_Vertical ';aim=[-6,0,4]
        if placement == 'Normal':
            circle=Rhino.Geometry.Circle(Rhino.Geometry.Point3d.Origin,3.).ToNurbsCurve()
            try: reference.append(document.Objects.AddCurve(circle))
            finally: circle.Dispose()
            if reference[0] == System.Guid.Empty: raise ValueError('translation preview reference insertion failed')
            prefix+='_Normal _Pause ';base=[3,0,0]
        for key in sources: document.Objects.Select(key)
        before=snapshot()
        prefix+='w'+','.join(str(v) for v in base)+' '
        if placement in ('Repeat','FromLastPoint','UseLastDistance','UseLastDirection'):
            prefix+='w-6,0,0 '
            if placement != 'Repeat': prefix+='_'+placement+'=_Yes '
        pixel=viewport.WorldToClient(host['_point'](aim));x,y=int(pixel.X),int(pixel.Y)
        if not 1<=x<viewport.Size.Width-1 or not 1<=y<viewport.Size.Height-1: raise ValueError('translation cursor outside viewport')
        screen=view.ClientToScreen(System.Drawing.Point(x,y));corner=view.ClientToScreen(System.Drawing.Point(0,0))
        if placement=='Normal':
            reference_client=viewport.WorldToClient(host['_point']([0,3,0]));reference_pixel=[int(reference_client.X),int(reference_client.Y)]
            if not 1<=reference_pixel[0]<viewport.Size.Width-1 or not 1<=reference_pixel[1]<viewport.Size.Height-1:
                raise ValueError('Move Normal preview reference outside viewport')
            reference_screen=view.ClientToScreen(System.Drawing.Point(*reference_pixel))
        metadata=dict(rect=[int(corner.X),int(corner.Y),int(corner.X+viewport.Size.Width),int(corner.Y+viewport.Size.Height)],
            aim_client=[float(pixel.X),float(pixel.Y)],valid_screen=[int(screen.X),int(screen.Y)],
            regions={label:[float(viewport.WorldToClient(host['_point'](p)).X),float(viewport.WorldToClient(host['_point'](p)).Y)]
                for label,p in [('source_point',[3,3,0]),('source_box',[3,-4,2]),('first_copy',[-3,3,0])]})
        with open(os.path.join(root,'translation-preview-'+operation['id']+'.json'),'w') as stream:json.dump(metadata,stream)
        doc_history=Rhino.RhinoApp.CommandHistoryWindowText
        with environment({},host):
            hooks_owned=True
            with _input_hooks(timer,Mouse(),[(timer.Tick,tick),(Rhino.Commands.Command.BeginCommand,begun),(Rhino.Commands.Command.EndCommand,ended)]):
                document.Views.Redraw()
                if placement=='Normal':send('PICK @translation-reference:'+operation['id']+' %d %d'%(reference_screen.X,reference_screen.Y))
                send('PICK @translation-preview:'+operation['id']+' %d %d'%(screen.X,screen.Y))
                succeeded=bool(Rhino.RhinoApp.RunScript(prefix+'_Pause'+(' _Enter' if operation['command']=='Copy' else ''),False))
        if errors or len(pending)!=1:raise ValueError('translation preview incomplete: '+str(errors))
        if placement=='Normal' and reference_click!=[reference_pixel]:raise ValueError('Move Normal preview reference was not clicked')
        after=snapshot()
        history=Rhino.RhinoApp.CommandHistoryWindowText
        history=history[len(doc_history):] if history.startswith(doc_history) else history[-2000:]
        value=dict(before=before,pending=pending[0],after=after,success=succeeded,history=history[-2000:],calibration=metadata)
        if placement=='Normal':value['reference_click']=reference_click[0]
        return value,0
    finally:
        if not hooks_owned:timer.Dispose()
        for obj in list(document.Objects):document.Objects.Delete(obj.Id,True)
        viewport.SetViewProjection(original,False);viewport.SetCameraTarget(target,False)
        viewport.Name=name;viewport.DisplayMode=mode;original.Dispose();document.Views.Redraw()
