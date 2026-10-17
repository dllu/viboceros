"""Completed public Rectangle getters with exact curve structure and history."""
import math,re
if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_source
else:
    from geometry_validity_probe import validate as validate_source


def validate(op):
    if not isinstance(op,dict)or op.get('op')!='rectangle'or set(op)-{'op','id','actions','construction_plane','curve','cancel_tail','preselect','setup'}or not {'id','actions'}<=set(op):raise ValueError('invalid rectangle recipe')
    if not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,100}\Z',op['id'])is None:raise ValueError('invalid rectangle id')
    def finite(n):return type(n)in(int,float)and not(math.isnan(n)or math.isinf(n))and abs(n)<=1e6
    def point(p):return isinstance(p,list)and len(p)==3 and all(finite(n)for n in p)
    if not isinstance(op['actions'],list)or not 1<=len(op['actions'])<=24:raise ValueError('invalid rectangle actions')
    for item in op['actions']:
        if isinstance(item,str):
            if len(item)>64:raise ValueError('rectangle option too long')
            if item not in('3Point','Center','Vertical','EdgeMidpoint','AroundCurve','Enter','Rounded','Corner','Arc','Conic','Corner=Arc','Corner=Conic','Rounded=Yes','Rounded=No')and not re.match(r'\A(?:Rho|Radius)=[0-9]+(?:\.[0-9]+)?\Z',item):raise ValueError('invalid rectangle option')
        elif not(point(item)or finite(item)):raise ValueError('invalid rectangle action')
    if 'construction_plane'in op and(not isinstance(op['construction_plane'],list)or len(op['construction_plane'])!=3 or not all(point(p)for p in op['construction_plane'])):raise ValueError('invalid rectangle frame')
    if 'curve'in op:
        validate_source(dict(op='geometry_validity',id=op['id'],source=op['curve']))
        if op['curve'].get('type')not in('line','nurbs','arc','circle','polyline','polycurve'):raise ValueError('invalid rectangle guide')
    if 'setup'in op:validate(dict(op='rectangle',id=op['id'],actions=op['setup']))
    for key in('cancel_tail','preselect'):
        if type(op.get(key,False))is not bool:raise ValueError('invalid rectangle flag')


def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('rectangle capture needs one iteration')
    from join_probe import observe_command
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport
    before=set(obj.Id for obj in doc.Objects);saved=vp.GetConstructionPlane();guide=None;geometry=None
    try:
        if 'construction_plane'in op:
            origin,x,y=op['construction_plane'];plane=G.Plane(G.Point3d(*origin),G.Vector3d(*x),G.Vector3d(*y))
            if not plane.IsValid:raise ValueError('invalid rectangle frame')
            vp.SetConstructionPlane(plane)
        else:vp.SetConstructionPlane(G.Plane.WorldXY)
        if 'curve'in op:
            geometry=host['_object_source'](op['curve'],tolerance);guide=doc.Objects.AddCurve(geometry)
        doc.Objects.UnselectAll()
        if guide and op.get('preselect'):doc.Objects.Select(guide,True)
        def curve_value(curve):
            pieces=list(curve.DuplicateSegments())if isinstance(curve,G.PolyCurve)else[curve.DuplicateCurve()]
            try:
                return dict(curve=host['_nurbs_curve_definition'](curve),segments=[host['_nurbs_curve_definition'](p)for p in pieces],length=float(curve.GetLength()),closed=bool(curve.IsClosed))
            finally:
                for p in pieces:p.Dispose()
        def snapshot():
            return dict(outputs=[dict(selected=bool(obj.IsSelected(False)),**curve_value(obj.Geometry))for obj in doc.Objects if obj.Id not in before and obj.Id!=guide],guide_unchanged=guide is None or doc.Objects.FindId(guide).Geometry.DataCRC(0)==geometry.DataCRC(0))
        def macro_for(actions):
            tokens=['_Rectangle']
            for action in actions:
                tokens.append('_'+action if isinstance(action,str)else'w'+','.join(repr(float(v))for v in action)if isinstance(action,list)else repr(float(action)))
                if action=='AroundCurve'and guide and not op.get('preselect'):tokens.extend(["'_-SelID",str(guide)])
            return ' '.join(tokens)
        if 'setup'in op:
            setup_before=set(obj.Id for obj in doc.Objects)
            setup_macro=macro_for(op['setup'])
            if not R.RhinoApp.RunScript(setup_macro,True):raise ValueError('rectangle setup failed')
            for obj in list(doc.Objects):
                if obj.Id not in setup_before:doc.Objects.Delete(obj.Id,True)
            doc.Objects.UnselectAll()
        tokens=[macro_for(op['actions'])]
        if op.get('cancel_tail'):tokens+=['_Cancel','_Cancel']
        macro=' '.join(tokens);marker='Owned Rectangle '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker);host['_record_progress'](op['id']+' macro: '+macro)
        doc.ClearUndoRecords(True);seed=snapshot()
        ok,after,events=observe_command(R.Commands.Command,'Rectangle',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        result=dict(command_returned=bool(ok),before=seed,after=after,after_script=snapshot(),events=events,macro=macro,history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip())
        result['undo_returned']=bool(R.RhinoApp.RunScript('_Undo',False));result['undo']=snapshot()
        result['redo_returned']=bool(R.RhinoApp.RunScript('_Redo',False));result['redo']=snapshot()
        return result,0
    finally:
        for obj in list(doc.Objects):
            if obj.Id not in before:doc.Objects.Delete(obj.Id,True)
        if geometry is not None:geometry.Dispose()
        vp.SetConstructionPlane(saved)
