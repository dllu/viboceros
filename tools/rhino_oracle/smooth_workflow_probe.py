"""Closed native Smooth preferences, admission, and trimmed-surface witnesses."""
import re
if __package__:
    from .smooth_probe import create_source, MODES
else:
    from smooth_probe import create_source, MODES

CASES = ('accepted','cancelled','numeric_prompt','toggles','undo_redo',
         'unsupported','mixed','trimmed_free','trimmed_fixed','polycurve',
         'cancel_factor','cancel_steps','cancel_coordinates','after_undo',
         'mesh','grips_curve','grips_surface','grips_mesh','line','replace',
         'partial_cancel','surface','grips_trimmed',
         'keyboard_escape','keyboard_factor','keyboard_selection')


def validate_request(q):
    if (not isinstance(q,dict) or type(q.get('protocol_version')) is not int or q['protocol_version']!=1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1)!=1
            or not isinstance(q.get('operations'),list) or not 1<=len(q['operations'])<=32):
        raise ValueError('Smooth workflows require bounded protocol 1 recipes')
    seen=set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op)!={'op','id','case'} or op['op']!='smooth_workflow'
                or not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None
                or op['id'] in seen or op['case'] not in CASES):
            raise ValueError('invalid Smooth workflow recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1,iterations=1,operations=[
        dict(op='smooth_workflow',id='smooth_workflow_'+case,case=case) for case in CASES])


def run(op,host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino,System=host['Rhino'],host['System'];doc=Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand(): raise ValueError('Smooth workflow requires idle execution')
    if list(doc.Objects): raise ValueError('Smooth workflow requires an empty owned document')
    from join_probe import observe_command
    vp=doc.Views.ActiveView.ActiveViewport;old=vp.GetConstructionPlane()
    plane=Rhino.Geometry.Plane(host['_point']([5.,-4.,2.]),Rhino.Geometry.Vector3d(1.,1.,0.),Rhino.Geometry.Vector3d(-1.,1.,2.))
    saved_tolerance=doc.ModelAbsoluteTolerance;doc.ModelAbsoluteTolerance=1e-7;vp.SetConstructionPlane(plane)
    ids=[]
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g=obj.Geometry;a=obj.Attributes
            row=dict(source=ids.index(obj.Id) if obj.Id in ids else None,kind=g.GetType().Name,
                     selected=bool(obj.IsSelected(False)),name=a.Name,grips_on=bool(obj.GripsOn),
                     attribute_text=a.GetUserString('Code'),geometry_text=g.GetUserString('Code'),
                     grips=[dict(index=int(p.Index),point=host['_xyz'](p.CurrentLocation),selected=bool(p.IsSelected(False))) for p in (obj.GetGrips() or [])])
            if isinstance(g,Rhino.Geometry.Curve):row['curve']=host['_nurbs_curve_definition'](g.ToNurbsCurve())
            elif isinstance(g,Rhino.Geometry.Brep):row['brep']=host['_interchange_brep_record'](g)
            elif isinstance(g,Rhino.Geometry.Point):row['point']=host['_xyz'](g.Location)
            elif isinstance(g,Rhino.Geometry.Mesh):row['mesh']=host['_polygon_mesh_value'](g)
            else:raise ValueError('unexpected Smooth workflow geometry')
            rows.append(row)
        return rows
    def invoke(macro,name='Smooth',keyboard=False):
        marker='Viboceros Smooth workflow '+str(System.Guid.NewGuid());Rhino.RhinoApp.WriteLine(marker)
        host['_record_progress'](op['id']+' '+macro)
        sent=[]
        def begun(sender,event):
            if event.CommandEnglishName=='Smooth':
                if Rhino.RhinoDoc.ActiveDoc!=doc:raise ValueError('foreign Smooth keyboard document')
                sent.append(dict(stage=op['case'],objects=snapshot()))
                host['_record_progress']('SMOOTH_ESCAPE '+op['id'])
        if keyboard:Rhino.Commands.Command.BeginCommand+=begun
        try:
            success,after,events=observe_command(Rhino.Commands.Command,name,
                lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        finally:
            if keyboard:Rhino.Commands.Command.BeginCommand-=begun
        if keyboard and len(sent)!=1:raise ValueError('Smooth keyboard command receipt missing')
        if Rhino.Commands.Command.InCommand():raise ValueError('Smooth workflow command remains active')
        return dict(macro=macro,success=success,after=after,after_script=snapshot(),events=events,keyboard=sent,
                    history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1])
    def repeat_cancel():
        doc.Objects.UnselectAll()
        for key in ids:doc.Objects.Select(key)
        before=snapshot();result=invoke('_-Smooth _Cancel')
        def geometry_only(rows):return [dict((k,v) for k,v in row.items() if k!='selected') for row in rows]
        result['geometry_unchanged']=geometry_only(snapshot())==geometry_only(before)
        result['before']=before
        return result
    try:
        seed=create_source('curve',host)
        try:seed_id=doc.Objects.Add(seed)
        finally:seed.Dispose()
        doc.Objects.Select(seed_id)
        if not Rhino.RhinoApp.RunScript('_-Smooth '+MODES['defaults']+' _Cancel _Cancel',False):
            raise ValueError('Smooth preference seed failed')
        doc.Objects.Delete(seed_id,True)
        case=op['case'];geometry=[]
        if case in ('unsupported','mixed','keyboard_selection'):
            if case=='mixed':geometry.append(create_source('curve',host))
            geometry.append(Rhino.Geometry.Point(host['_point']([8.,1.,0.])))
            geometry.append(Rhino.Geometry.Brep.CreateFromBox(Rhino.Geometry.BoundingBox(host['_point']([10.,0.,0.]),host['_point']([12.,2.,2.]))))
        elif case.startswith('trimmed') or case=='grips_trimmed':
            surface=create_source('surface',host)
            brep=surface.ToBrep();uv=Rhino.Geometry.Circle(Rhino.Geometry.Plane.WorldXY,.3).ToNurbsCurve()
            edge=None;split=None
            try:
                uv.Transform(Rhino.Geometry.Transform.Translation(.5,.5,0.))
                edge=surface.Pushup(uv,doc.ModelAbsoluteTolerance)
                if edge is None:raise ValueError('Smooth trim pushup failed')
                split=brep.Faces[0].Split([edge],doc.ModelAbsoluteTolerance)
                if split is None:raise ValueError('Smooth trim split failed')
                pieces=[face.DuplicateFace(False) for face in split.Faces if face.Loops.Count==1]
                if len(pieces)!=1:raise ValueError('Smooth trim interior face is ambiguous')
                geometry.extend(pieces)
            finally:
                if split is not None:split.Dispose()
                if edge is not None:edge.Dispose()
                uv.Dispose();brep.Dispose();surface.Dispose()
        elif case=='polycurve':
            pc=Rhino.Geometry.PolyCurve()
            line=Rhino.Geometry.LineCurve(host['_point']([-2.,0.,0.]),host['_point']([0.,0.,0.]))
            arc=Rhino.Geometry.ArcCurve(Rhino.Geometry.Arc(host['_point']([0.,0.,0.]),host['_point']([1.,1.,0.]),host['_point']([2.,0.,0.])))
            try:
                if not pc.Append(line) or not pc.Append(arc):raise ValueError('Smooth polycurve construction failed')
            finally:line.Dispose();arc.Dispose()
            geometry.append(pc)
        else:geometry.append(create_source(dict(mesh='mesh_grid',grips_curve='curve',grips_surface='surface',grips_mesh='mesh_grid',line='line',surface='surface').get(case,'curve'),host))
        serial=doc.BeginUndoRecord('Smooth workflow sources')
        try:
            for i,g in enumerate(geometry):
                a=Rhino.DocObjects.ObjectAttributes();a.Name='smooth workflow '+str(i);a.SetUserString('Code','attribute');g.SetUserString('Code','geometry')
                try:key=doc.Objects.Add(g,a)
                finally:g.Dispose();a.Dispose()
                if key==System.Guid.Empty:raise ValueError('Smooth workflow source insertion failed')
                ids.append(key)
        finally:doc.EndUndoRecord(serial)
        for key in ids:doc.Objects.Select(key)
        if case.startswith('grips_'):
            owner=doc.Objects.FindId(ids[0]);owner.GripsOn=True
            for p in owner.GetGrips():
                if int(p.Index) in (0,1):p.Select(True)
        before=snapshot()
        tail=MODES['defaults' if case=='trimmed_fixed' else 'free']
        if case in ('accepted','undo_redo'):
            tail='_SmoothFactor=.35 _Steps=3 _CoordinateSystem=_CPlane _X=_Yes _Y=_No _Z=_No _FixBoundaries=_No _Enter'
        elif case=='cancelled':
            tail='_SmoothFactor=.7 _Steps=2 _CoordinateSystem=_Object _X=_No _Y=_Yes _Z=_No _FixBoundaries=_No _Cancel'
        elif case=='numeric_prompt':tail='_SmoothFactor .4 _Steps 2 _Enter'
        elif case=='toggles':tail='_X _Y _Z _FixBoundaries _Enter'
        elif case=='cancel_factor':tail='_SmoothFactor _Cancel'
        elif case=='cancel_steps':tail='_Steps _Cancel'
        elif case=='cancel_coordinates':tail='_CoordinateSystem _Cancel'
        elif case=='replace':tail='_SmoothFactor=.4 ! _Line w10,0,0 w12,0,0'
        elif case=='partial_cancel':tail='_SmoothFactor=.4 _X _Steps _Cancel'
        elif case=='keyboard_escape':tail='_SmoothFactor=.4 _Pause'
        elif case=='keyboard_factor':tail='_SmoothFactor _Pause'
        elif case=='keyboard_selection':tail='_Pause'
        keyboard=case.startswith('keyboard')
        result=invoke('_-Smooth '+tail+('' if keyboard else ' _Cancel _Cancel'),keyboard=keyboard)
        result['before']=before;result['plane']=dict(origin=host['_xyz'](plane.Origin),x_axis=host['_xyz'](plane.XAxis),y_axis=host['_xyz'](plane.YAxis))
        if case=='undo_redo':
            result['undo']=invoke('_Undo','Undo')
            result['redo']=invoke('_Redo','Redo')
        elif case=='after_undo':result['undo']=invoke('_Undo','Undo')
        if case not in ('unsupported','keyboard_selection'):result['followup_cancel']=repeat_cancel()
        return result,0
    finally:
        for obj in list(doc.Objects):
            if obj.GripsOn:obj.GripsOn=False
            doc.Objects.Delete(obj.Id,True)
        vp.SetConstructionPlane(old);doc.ModelAbsoluteTolerance=saved_tolerance;doc.Views.Redraw()
