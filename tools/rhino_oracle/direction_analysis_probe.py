"""Owned ShowDir command and public object-analysis state observations."""
import os
import re

SOURCES=('warped','holed','box','sphere','line','mesh','point')
STEPS=('ShowDir','ShowDirOff','Hide','Show','Delete','Undo','Redo','Flip','UReverse','VReverse','SwapUV',
       'Direction=No','Direction=Yes','U=No','U=Yes','V=No','V=Yes')
STEPS += ('SelectPrimary','SelectCompanion','SelNone','SelAll','Add','Remove')

def validate(op):
    if isinstance(op,dict) and op.get('op')=='subd_direction_analysis':
        if __package__:from .subd_direction_analysis_cases import validate as owned_validate
        else:from subd_direction_analysis_cases import validate as owned_validate
        owned_validate(op);return
    if (not isinstance(op,dict) or set(op)-{'companion','picks'} != {'op','id','source','preselect','steps'}
            or op['op']!='direction_analysis' or op['source'] not in SOURCES
            or type(op['preselect']) is not bool or not isinstance(op['id'],str)
            or re.match(r'^[A-Za-z0-9_-]{1,100}\Z',op['id']) is None
            or type(op.get('companion',False)) is not bool
            or not isinstance(op['steps'],list) or not 1 <= len(op['steps']) <= 24
            or any(step not in STEPS for step in op['steps'])):
        raise ValueError('invalid direction analysis recipe')
    if 'picks' in op and (not isinstance(op['picks'],dict) or any(not isinstance(k,str) or not k.isdigit() or not 0<=int(k)<len(op['steps']) or type(v) is not int or not 0<=v<(2 if op.get('companion',False) else 1) for k,v in op['picks'].items())):raise ValueError('invalid ShowDir getter targets')

def controls(action=None):
    import ctypes
    from ctypes import wintypes
    user=ctypes.windll.user32
    callback=ctypes.WINFUNCTYPE(wintypes.BOOL,wintypes.HWND,wintypes.LPARAM)
    user.EnumWindows.argtypes=[callback,wintypes.LPARAM]
    user.EnumChildWindows.argtypes=[wintypes.HWND,callback,wintypes.LPARAM]
    user.GetWindowThreadProcessId.argtypes=[wintypes.HWND,ctypes.POINTER(wintypes.DWORD)]
    user.GetWindowTextW.argtypes=[wintypes.HWND,wintypes.LPWSTR,ctypes.c_int]
    user.GetClassNameW.argtypes=[wintypes.HWND,wintypes.LPWSTR,ctypes.c_int]
    user.IsWindowVisible.argtypes=[wintypes.HWND]
    user.SendMessageW.argtypes=[wintypes.HWND,wintypes.UINT,wintypes.WPARAM,wintypes.LPARAM]
    rows=[];buttons=[]
    def text(window):
        label=ctypes.create_unicode_buffer(512);kind=ctypes.create_unicode_buffer(128)
        user.GetWindowTextW(window,label,512);user.GetClassNameW(window,kind,128)
        return label.value,kind.value
    def child(window,unused):
        label,kind=text(window)
        rows.append(dict(label=label,kind=kind,check=int(user.SendMessageW(window,0x00F0,0,0)) if kind=='Button' else None))
        if kind=='Button':buttons.append((label.replace('&',''),window))
        return True
    def visit(window,unused):
        pid=wintypes.DWORD();user.GetWindowThreadProcessId(window,ctypes.byref(pid))
        label,kind=text(window)
        if pid.value==ctypes.windll.kernel32.GetCurrentProcessId() and user.IsWindowVisible(window) and 'Direction' in label:
            rows.append(dict(label=label,kind=kind,check=None))
            user.EnumChildWindows(window,callback(child),0)
        return True
    user.EnumWindows(callback(visit),0)
    if action is not None:
        label=action.split('=')[0] if '=' in action else dict(Flip='Flip Direction',UReverse='Reverse U',VReverse='Reverse V',SwapUV='Swap UV',Add='Add Objects',Remove='Remove Objects')[action]
        matches=[window for text,window in buttons if text==label]
        if len(matches)!=1:raise ValueError('owned direction-analysis control unavailable: '+label)
        if '=' not in action or bool(user.SendMessageW(matches[0],0x00F0,0,0)) != action.endswith('=Yes'):
            user.SendMessageW(matches[0],0x00F5,0,0)
    return rows

def run(op,host,iterations,tolerance):
    validate(op)
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    from join_probe import observe_command
    if iterations!=1 or list(doc.Objects) or doc.UndoRecordingIsActive or R.Commands.Command.InCommand():
        raise ValueError('ShowDir capture requires an empty owned idle document')
    model=None;source=None;key=S.Guid.Empty;keys=[];finish=None
    old_appearance=doc.SubDAppearance
    if op['op']=='subd_direction_analysis':
        from subd_direction_cases import source as subd_recipe
        from subd_to_nurbs_probe import make_source
        source=make_source(subd_recipe(op['source'][5:]),host)
        doc.SubDAppearance=G.SubDComponentLocation.ControlNet if op['flat'] else G.SubDComponentLocation.Surface
    elif op['source'] in ('line','mesh','point'):
        if op['source']=='line': source=G.LineCurve(G.Point3d(0,0,0),G.Point3d(4,2,1))
        elif op['source']=='point':source=G.Point(G.Point3d(1,2,3))
        else:
            source=G.Mesh()
            for p in [(0,0,0),(4,0,0),(4,3,1),(0,3,0)]:source.Vertices.Add(*p)
            source.Faces.AddFace(0,1,2,3)
    else:
        model=R.FileIO.File3dm.Read(os.path.join(os.path.dirname(__file__),'fixtures','brep_direction',op['source']+'.3dm'))
        if model is None or model.Objects.Count!=1:raise ValueError('shared ShowDir operand unavailable')
        source=list(model.Objects)[0].Geometry.DuplicateBrep()
    def object_snapshot(identity):
        obj=doc.Objects.FindId(identity)
        if obj is None:return dict(exists=False,selected=False,hidden=False,analysis=False,geometry=None)
        g=obj.Geometry
        if isinstance(g,G.SubD):
            from subd_direction_probe import geometry as subd_geometry
            geometry=subd_geometry(g,host)
        else:geometry=(host['_interchange_brep_record'](g) if isinstance(g,G.Brep) else
                  host['_interchange_curve_record'](g) if isinstance(g,G.Curve) else
                  host['_polygon_mesh_value'](g) if isinstance(g,G.Mesh) else [float(g.Location.X),float(g.Location.Y),float(g.Location.Z)])
        return dict(exists=True,selected=bool(obj.IsSelected(False)),hidden=bool(obj.IsHidden),
            analysis=bool(obj.InVisualAnalysisMode()),geometry=geometry)
    def snapshot():
        rows=[object_snapshot(identity) for identity in keys]
        return rows if op.get('companion',False) else rows[0]
    try:
        R.RhinoApp.RunScript('_ShowDirOff',False)
        key=doc.Objects.AddBrep(source,None,None,False,False) if isinstance(source,G.Brep) else doc.Objects.Add(source)
        if key==S.Guid.Empty:raise ValueError('ShowDir source insertion failed')
        keys.append(key)
        if op.get('companion',False):
            companion=doc.Objects.AddBrep(source,None,None,False,False) if isinstance(source,G.Brep) else doc.Objects.Add(source)
            if companion==S.Guid.Empty:raise ValueError('ShowDir companion insertion failed')
            keys.append(companion)
        if op['preselect']:doc.Objects.Select(key)
        doc.ClearUndoRecords(True)
        states=[snapshot()];traces=[]
        for step_index,step in enumerate(op['steps']):
            finish=None
            if 'picks' in op:
                from direction_analysis_input import start
                pick=op['picks'].get(str(step_index),0)
                finish=start(R,S,doc,keys[pick],pick,step)
            select='' if any(obj.IsSelected(False) for obj in doc.Objects) else ' _SelID '+str(key)+' _Enter'
            macro=('_ShowDir'+select if step=='ShowDir' else ('_SelID '+str(key)+' _Delete' if step=='Delete' else '_'+step+(select if step=='Hide' else '')))
            if op['op']=='subd_direction_analysis' and step=='ShowDir':macro+=' !'
            if step in ('SelectPrimary','SelectCompanion','SelNone'):
                doc.Objects.UnselectAll()
                if step!='SelNone':
                    index=1 if step=='SelectCompanion' else 0
                    if index>=len(keys):raise ValueError('ShowDir companion selection unavailable')
                    doc.Objects.Select(keys[index])
                success=True;state=snapshot();events=[];macro=None
            elif step in ('Flip','UReverse','VReverse','SwapUV','Add','Remove') or '=' in step:
                events=[]
                def ended(sender,args):events.append(dict(name=str(args.CommandEnglishName),result=str(args.CommandResult)))
                R.Commands.Command.EndCommand+=ended
                try:controls(step);R.RhinoApp.Wait()
                finally:R.Commands.Command.EndCommand-=ended
                success=True;state=snapshot();macro=None
            else:
                success,state,events=observe_command(R.Commands.Command,step,lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
                if step=='ShowDir' and (op['op']!='subd_direction_analysis' or controls()):
                    for setting in ('Direction=Yes','U=Yes','V=Yes'):controls(setting)
            R.RhinoApp.Wait()
            modes=[];obj=doc.Objects.FindId(key)
            if obj is not None:modes=[dict(name=m.Name,id=str(m.Id)) for m in obj.GetActiveVisualAnalysisModes()]
            traces.append(dict(command=step,macro=macro,success=success,events=events,controls=controls(),modes=modes))
            if finish is not None:traces[-1]['input']=finish()
            states.append(state)
        return dict(states=states,traces=traces),0
    finally:
        if finish is not None:finish.stop()
        if R.Commands.Command.InCommand():R.RhinoApp.RunScript('!',False)
        R.RhinoApp.RunScript('_ShowDirOff',False)
        for identity in keys:doc.Objects.Delete(identity,True)
        if source is not None:source.Dispose()
        if model is not None:model.Dispose()
        doc.SubDAppearance=old_appearance
