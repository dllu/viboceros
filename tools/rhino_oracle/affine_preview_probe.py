"""Bounded native Scale/Rotate/Shear cursor previews in an empty owned document."""
import json
import os
import re

COMMANDS=('Scale','Scale1D','Scale2D','Rotate','Rotate3D','Shear')


def validate_request(request):
    if (not isinstance(request,dict) or type(request.get('protocol_version')) is not int
            or request['protocol_version']!=1 or type(request.get('iterations',1)) is not int
            or request.get('iterations',1)!=1 or not isinstance(request.get('operations'),list)
            or not 1<=len(request['operations'])<=64):
        raise ValueError('affine preview requires bounded protocol 1 cases')
    names=set()
    for op in request['operations']:
        if (not isinstance(op,dict) or set(op)!={'op','id','command','copy','phase','view','display_mode','cursor','finish'}
                or op['op']!='affine_preview' or not isinstance(op['id'],str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None or op['id'] in names
                or op['command'] not in COMMANDS or type(op['copy']) is not bool
                or op['phase'] not in ('Target','Repeat','Direction','ZeroDirection') or op['view'] not in ('Top','Perspective')
                or op['display_mode'] not in ('Wireframe','Shaded','Ghosted')
                or op['cursor'] not in ('Valid','Degenerate') or op['finish'] not in ('Click','Cancel')
                or op['phase']=='Repeat' and not op['copy']
                or op['phase'] in ('Direction','ZeroDirection') and op['command']!='Scale1D'
                or op['cursor']=='Degenerate' and op['finish']!='Cancel'):
            raise ValueError('invalid affine preview case')
        names.add(op['id'])


def request():
    cases=[]
    def add(command,copy=False,phase='Target',view='Top',mode='Shaded',cursor='Valid',finish='Click'):
        cases.append(dict(op='affine_preview',id='_'.join((command,'Copy' if copy else 'Move',phase,view,mode,cursor,finish)),
            command=command,copy=copy,phase=phase,view=view,display_mode=mode,cursor=cursor,finish=finish))
    for mode in ('Wireframe','Shaded','Ghosted'):
        for command in COMMANDS:
            for copy in (False,True):add(command,copy,mode=mode)
    for command in COMMANDS:
        add(command,True,'Repeat')
        add(command,mode='Ghosted',view='Perspective')
        add(command,mode='Shaded',cursor='Degenerate',finish='Cancel')
    for mode in ('Wireframe','Shaded','Ghosted'):add('Scale1D',True,'Direction',mode=mode)
    for mode in ('Wireframe','Shaded','Ghosted'):add('Scale1D',True,'ZeroDirection',mode=mode,finish='Cancel')
    return dict(protocol_version=1,iterations=1,operations=cases)


def recipe(op):
    command=op['command'];copy='_Copy=_'+('Yes' if op['copy'] else 'No')+' '
    reference=dict(Scale=[4,0,0],Scale1D=[4,0,0],Scale2D=[4,0,2],Rotate=[5,0,0],Rotate3D=[5,-1,0],Shear=[4,1,2])[command]
    aim=dict(Scale=[2,5,0],Scale1D=[2,5,0],Scale2D=[2,5,0],Rotate=[0,5,0],Rotate3D=[-1,5,0],Shear=[2,5,0])[command]
    base='w0,0,0 '
    if command=='Rotate3D':prefix='_Rotate3D '+base+'w1,1,2 '+copy
    else:prefix='_'+command+' '+copy+base
    factor=0. if op['phase']=='ZeroDirection' else -2.
    if op['phase'] in ('Direction','ZeroDirection'):prefix+=str(factor)+' '
    else:prefix+='w'+','.join(str(v) for v in reference)+' '
    if op['phase']=='Repeat':prefix+='w2,2,0 '
    return dict(prefix=prefix,aim=aim,reference=reference,axis=[1,1,2],factor=factor)


def run(op,host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino,System=host['Rhino'],host['System'];doc=Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):raise ValueError('affine preview requires an empty owned document')
    import clr
    clr.AddReference('System.Windows.Forms')
    from System.Windows.Forms import Timer
    from shrink_face_input import _input_hooks
    from snap_environment import environment
    from viewport_capture import capture
    from named_view_policy_probe import snapshot as camera_snapshot
    view=doc.Views.ActiveView;vp=view.ActiveViewport;original=Rhino.DocObjects.ViewportInfo(vp)
    name,mode,target=vp.Name,vp.DisplayMode,vp.CameraTarget
    root=os.path.dirname(os.path.abspath(host['__file__']));progress=os.path.join(root,'worker-progress.log')
    ready=os.path.join(root,'affine-preview-ready-'+op['id']+'.json')
    pending,errors,sources,active,witness=[],[],[],[],[];timer=Timer();timer.Interval=100;hooks_owned=False
    def snapshot():
        rows=[]
        for obj in doc.Objects:
            geom=obj.Geometry;vertices=None;points=None
            if isinstance(geom,Rhino.Geometry.Brep):vertices=[host['_xyz'](v.Location) for v in geom.Vertices]
            elif isinstance(geom,Rhino.Geometry.Point):points=[host['_xyz'](geom.Location)]
            elif isinstance(geom,Rhino.Geometry.Curve):
                ok,polyline=geom.TryGetPolyline()
                if ok:points=[host['_xyz'](p) for p in polyline]
            box=geom.GetBoundingBox(True)
            rows.append(dict(source=sources.index(obj.Id) if obj.Id in sources else None,witness=obj.Id in witness,selected=bool(obj.IsSelected(False)),
                type=str(geom.ObjectType),bounds=[host['_xyz'](box.Min),host['_xyz'](box.Max)],vertices=vertices,points=points))
        return rows
    def send(line):
        with open(progress,'a') as stream:stream.write(line+'\n');stream.flush()
    def begun(sender,event):
        if event.CommandEnglishName==op['command']:active.append(True)
    def ended(sender,event):
        if event.CommandEnglishName==op['command']:active[:]=[]
    class Mouse(Rhino.UI.MouseCallback):
        def OnEndMouseMove(self,event):
            if (not active or pending or errors or event.View.ActiveViewport.Id!=vp.Id
                    or [int(event.ViewportPoint.X),int(event.ViewportPoint.Y)]!=[x,y]
                    or not Rhino.Input.RhinoGet.InGetPoint(doc)):return
            try:
                ok,ray=vp.GetFrustumLine(x,y)
                if not ok:raise ValueError('affine preview ray unavailable')
                frame=capture(vp,host['_point'](aim),[x,y],host);frame['ray']=[host['_xyz'](ray.From),host['_xyz'](ray.To)]
                pending.append(dict(objects=snapshot(),frame=frame,camera=camera_snapshot(vp,Rhino),prompt=Rhino.RhinoApp.CommandPrompt))
                with open(ready+'.tmp','w') as stream:json.dump(op['id'],stream)
                os.rename(ready+'.tmp',ready)
            except Exception as error:errors.append(str(error));send('PICK_ABORT affine-preview-'+op['id'])
    try:
        if not vp.SetProjection(getattr(Rhino.Display.DefinedViewportProjection,op['view']),'Owned affine preview',False):
            raise ValueError('affine preview projection failed')
        vp.SetConstructionPlane(Rhino.Geometry.Plane.WorldXY)
        vp.DisplayMode=Rhino.Display.DisplayModeDescription.FindByName(op['display_mode'])
        if not vp.ZoomBoundingBox(Rhino.Geometry.BoundingBox(host['_point']([-14,-10,-5]),host['_point']([14,12,12]))):
            raise ValueError('affine preview zoom failed')
        attrs=Rhino.DocObjects.ObjectAttributes();attrs.ColorSource=Rhino.DocObjects.ObjectColorSource.ColorFromObject
        attrs.ObjectColor=System.Drawing.Color.FromArgb(200,80,60)
        sources.append(doc.Objects.AddPoint(host['_point']([3,3,0]),attrs))
        curve=Rhino.Geometry.PolylineCurve([host['_point'](p) for p in ([2,1,0],[4,1,0],[3,-1,0],[2,1,0])])
        try:sources.append(doc.Objects.AddCurve(curve,attrs))
        finally:curve.Dispose()
        box=Rhino.Geometry.Brep.CreateFromBox(Rhino.Geometry.BoundingBox(host['_point']([2,-5,0]),host['_point']([4,-3,2])))
        try:sources.append(doc.Objects.AddBrep(box,attrs))
        finally:box.Dispose();attrs.Dispose()
        if any(key==System.Guid.Empty for key in sources):raise ValueError('affine preview source insertion failed')
        if op['cursor']=='Degenerate':
            witness.append(doc.Objects.AddPoint(Rhino.Geometry.Point3d.Origin))
            if witness[0]==System.Guid.Empty:raise ValueError('affine preview center witness insertion failed')
        for key in sources:doc.Objects.Select(key)
        before=snapshot();spec=recipe(op);aim=[0,0,0] if op['cursor']=='Degenerate' else spec['aim']
        pixel=vp.WorldToClient(host['_point'](aim));x,y=int(pixel.X),int(pixel.Y)
        if not 1<=x<vp.Size.Width-1 or not 1<=y<vp.Size.Height-1:raise ValueError('affine cursor outside viewport')
        screen=view.ClientToScreen(System.Drawing.Point(x,y));corner=view.ClientToScreen(System.Drawing.Point(0,0))
        valid=vp.WorldToClient(host['_point'](spec['aim']));valid_screen=view.ClientToScreen(System.Drawing.Point(int(valid.X),int(valid.Y)))
        metadata=dict(rect=[int(corner.X),int(corner.Y),int(corner.X+vp.Size.Width),int(corner.Y+vp.Size.Height)],
            aim_client=[float(pixel.X),float(pixel.Y)],valid_screen=[int(valid_screen.X),int(valid_screen.Y)],
            regions={label:[float(vp.WorldToClient(host['_point'](p)).X),float(vp.WorldToClient(host['_point'](p)).Y)]
                for label,p in [('source_point',[3,3,0]),('source_box',[3,-4,2])]})
        with open(os.path.join(root,'affine-preview-'+op['id']+'.json'),'w') as stream:json.dump(metadata,stream)
        history=Rhino.RhinoApp.CommandHistoryWindowText
        with environment(dict(persistent_snaps=['Point'] if witness else []),host):
            hooks_owned=True
            with _input_hooks(timer,Mouse(),[(Rhino.Commands.Command.BeginCommand,begun),(Rhino.Commands.Command.EndCommand,ended)]):
                doc.Views.Redraw();send('PICK @affine-preview:'+op['id']+' %d %d'%(screen.X,screen.Y))
                success=bool(Rhino.RhinoApp.RunScript(spec['prefix']+'_Pause'+(' _Enter' if op['copy'] else ''),False))
        if errors or len(pending)!=1:raise ValueError('affine preview incomplete: '+str(errors))
        after=snapshot();after_history=Rhino.RhinoApp.CommandHistoryWindowText
        delta=after_history[len(history):] if after_history.startswith(history) else after_history[-2000:]
        return dict(before=before,pending=pending[0],after=after,success=success,history=delta[-2000:],calibration=metadata),0
    finally:
        if not hooks_owned:timer.Dispose()
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        vp.SetViewProjection(original,False);vp.SetCameraTarget(target,False);vp.Name=name;vp.DisplayMode=mode
        original.Dispose();doc.Views.Redraw()
