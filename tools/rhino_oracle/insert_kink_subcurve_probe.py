"""Additional public InsertKink getters, reusing the retained source recipes."""
import math
import re
try:
    from . import insert_kink_probe
except (ImportError,ValueError):
    import insert_kink_probe

def validate(op):
    if not isinstance(op,dict)or 'getter'not in op:raise ValueError('missing temporary getter')
    plain=dict(op);getter=plain.pop('getter');plain['op']='insert_kink_command';plain['subcurve']=[.1,.9];plain['preselect']=False
    insert_kink_probe.validate(plain)
    if op['op']!='insert_kink_subcurve' or op['family']not in('line','cubic','rational','circle','ellipse','polyline','polycurve','periodic'):raise ValueError('invalid temporary curve')
    allowed={'start','end','lengths','confirm','lock_hover','unlock','cancel_getter','toggle_unlock','script_lock'}
    if not isinstance(getter,dict)or 'start'not in getter or set(getter)-allowed:raise ValueError('invalid temporary input fields')
    for key in('start','end','confirm','lock_hover'):
        if key in getter and(type(getter[key])not in(int,float)or not 0<=getter[key]<=1):raise ValueError('invalid getter station')
    for key in('unlock','cancel_getter','toggle_unlock','script_lock'):
        if type(getter.get(key,False))is not bool:raise ValueError('invalid temporary flag')
    lengths=getter.get('lengths',[])
    if not isinstance(lengths,list)or len(lengths)>4 or any(x!='Whole'and(type(x)not in(int,float)or math.isnan(x)or math.isinf(x)or abs(x)>1e6)for x in lengths):raise ValueError('invalid temporary lengths')
    if 'end'in getter and(lengths or 'confirm'in getter):raise ValueError('ambiguous temporary endpoint')
    if (getter.get('unlock')or getter.get('toggle_unlock'))and'lock_hover'not in getter:raise ValueError('unlock requires owned hover')

class Forward:
    def __init__(self,target):self.target=target
    def __getattr__(self,key):return getattr(self.target,key)

def run(op,host,iterations,tolerance):
    validate(op)
    R,S=host['Rhino'],host['System'];getter=op['getter'];app=Forward(R.RhinoApp);rhino=Forward(R);rhino.RhinoApp=app
    plain=dict(op);plain.pop('getter');plain.update(op='insert_kink_command',subcurve=[.1,.9],preselect=False)
    state=dict(tokens=[],macro=None,hover=None,typed=[],prompts=[])
    def execute(macro,echo):
        if not macro.startswith('_InsertKink '):return R.RhinoApp.RunScript(macro,echo)
        key=S.Guid(re.search(r'_SelID ([0-9a-f-]+)',macro).group(1));source=R.RhinoDoc.ActiveDoc.Objects.FindId(key).Geometry
        def point(f):return 'w'+','.join('%.17g'%x for x in host['_xyz'](source.PointAt(source.Domain.ParameterAt(f))))
        tokens=[]
        for length in getter.get('lengths',[]):tokens.append('%.17g'%(source.GetLength()if length=='Whole'else length))
        if 'confirm'in getter:tokens.append(point(getter['confirm']))
        if 'end'in getter:tokens.append(point(getter['end']))
        if getter.get('cancel_getter'):tokens.extend(['!','_Enter','!'])
        else:
            tokens.extend(point(f)for f in op['stations'])
            tokens.append('!'if op.get('cancel')else'_Enter')
            if not op.get('cancel'):tokens.append('!')
        state['tokens']=tokens;prefix='_InsertKink _SubCrv _SelID '+str(key)+' '+point(getter['start'])+' '
        doc=R.RhinoDoc.ActiveDoc;view=doc.Views.ActiveView;vp=view.ActiveViewport
        original=R.DocObjects.ViewportInfo(vp);plane=vp.GetConstructionPlane()
        from snap_environment import environment
        try:
            vp.SetProjection(R.Display.DefinedViewportProjection.Top,'Owned kink subcurve',False);vp.SetConstructionPlane(R.Geometry.Plane.WorldXY)
            bounds=source.GetBoundingBox(True);bounds.Inflate(1.);vp.ZoomBoundingBox(bounds);doc.Views.Redraw()
            with environment(dict(persistent_snaps=[]),host):
                if getter.get('cancel_getter'):return drive_cancel(prefix,tokens,source,view,vp,echo)
                if 'lock_hover'in getter:return drive_locked(prefix,tokens,source,view,vp,echo)
                state['macro']=prefix+' '.join(tokens)
                return R.RhinoApp.RunScript(state['macro'],echo)
        finally:
            vp.SetViewProjection(original,False);vp.SetConstructionPlane(plane);original.Dispose()
    def drive_cancel(prefix,tokens,source,view,vp,echo):
        import os,json
        root=os.path.dirname(host['__file__']);numeric=tokens[:-3];completed=[]
        state['tokens']=numeric+['Escape','Escape'];state['typed']=[dict(token=t,prompt='curve length')for t in numeric]
        state['macro']=prefix+' '.join(numeric)+' _Pause'
        def ended(sender,event):
            if event.CommandEnglishName!='InsertKink':return
            completed.append(str(event.CommandResult))
            with open(os.path.join(root,'kink-finished-'+op['id']+'.json'),'w')as f:json.dump(op['id'],f)
        event=R.Commands.Command.EndCommand;event+=ended
        try:
            host['_record_progress']('PICK @subcurve-direction:%s 1 1'%op['id'])
            success=R.RhinoApp.RunScript(state['macro'],echo)
            state['typed'].extend([dict(token='Escape',prompt='Select curve'),dict(token='Escape',prompt='Select curve or surface')])
            if os.path.exists(os.path.join(root,'kink-returned-'+op['id']+'.json')):
                state['tokens'].append('_Enter');state['typed'].append(dict(token='_Enter',prompt='Select curve or surface'))
            state['completed']=completed;R.RhinoApp.Wait()
            return success
        finally:event-=ended
    def drive_locked(prefix,tokens,source,view,vp,echo):
        import clr
        clr.AddReference('System.Windows.Forms')
        from System.Windows.Forms import Cursor
        aim=source.PointAt(source.Domain.ParameterAt(getter['lock_hover']));state['hover']=host['_xyz'](aim)
        pixel=vp.WorldToClient(aim);screen=view.ClientToScreen(S.Drawing.Point(int(pixel.X),int(pixel.Y)))
        saved=Cursor.Position
        options=['_Direction=_Locked']+(['_Direction=_Free']if getter.get('unlock')or getter.get('toggle_unlock')else[])
        state['tokens']=['Direction=Locked']+(['Direction=Free']if getter.get('unlock')or getter.get('toggle_unlock')else[])+tokens
        class Mouse(R.UI.MouseCallback):
            def OnEndMouseMove(self,event):
                if event.View.ActiveViewport.Id==vp.Id and abs(int(event.ViewportPoint.X)-int(pixel.X))<=1 and abs(int(event.ViewportPoint.Y)-int(pixel.Y))<=1:
                    state['motion']=[dict(pixel=[int(event.ViewportPoint.X),int(event.ViewportPoint.Y)],point=state['hover'])]
        mouse=Mouse();mouse.Enabled=True
        try:
            Cursor.Position=S.Drawing.Point(screen.X+1,screen.Y);Cursor.Position=screen;R.RhinoApp.Wait()
            state['cursor']=[int(Cursor.Position.X),int(Cursor.Position.Y)]
            if state['cursor']!=[int(screen.X),int(screen.Y)]:raise ValueError('owned cursor position was not applied')
            state['macro']=prefix+' '.join(options+tokens)
            host['_record_progress']('MACRO_LOCK '+op['id'])
            return R.RhinoApp.RunScript(state['macro'],echo)
        finally:
            mouse.Enabled=False;Cursor.Position=saved
    app.RunScript=execute
    local=dict(host);local['Rhino']=rhino
    original_record=host['_interchange_curve_record']
    def record_curve(curve):
        if not curve.IsValid:
            d=curve.Domain
            return dict(type='invalid_curve',valid=False,native_type=str(curve.GetType().Name),domain=[float(d.T0),float(d.T1)])
        return original_record(curve)
    local['_interchange_curve_record']=record_curve
    value,elapsed=insert_kink_probe.run(plain,local,iterations,tolerance)
    value['recipe']=op;value['macro']=state['macro'];value['input_trace']=state
    return value,elapsed
