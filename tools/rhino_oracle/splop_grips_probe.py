"""Owned Splop grip commands retain full owners, display, groups and history."""
import math
import re
try:
    from . import grip_transform_probe as source
except (ImportError,ValueError):
    import grip_transform_probe as source


def validate(op):
    required={'op','id','source','selected','copy','rigid','flip','surface'}
    optional={'edit','parent','point','grouped','targets','finish'}
    if (not isinstance(op,dict) or set(op)-optional!=required or op['op']!='splop_grips' or
            not isinstance(op['id'],str) or re.match(r'\A[A-Za-z0-9_-]{1,80}\Z',op['id']) is None or
            op['source'] not in source.KINDS or op['surface'] not in ('Plane','Cylinder') or
            any(type(op[k])is not bool for k in ('copy','rigid','flip')) or
            any(type(op.get(k,False))is not bool for k in ('edit','parent','point','grouped')) or
            op.get('finish','Complete')not in ('Complete','Cancel')):
        raise ValueError('invalid bounded Splop grips')
    source.validate(dict(op='grip_transform',id=op['id'],source=op['source'],selected=op['selected'],parent=op.get('parent',False),point=op.get('point',False),command='move',off=False,inputs='auto'))
    if op.get('edit') and op['source']not in ('curve','rational','closed','periodic'):
        raise ValueError('edit grips require a NURBS curve')
    targets=op.get('targets',[])
    if (not isinstance(targets,list) or len(targets)>3 or targets and not op['copy'] or
            any(not isinstance(t,list)or len(t)!=2 or any(type(v)not in(int,float)or math.isnan(v)or math.isinf(v)for v in t)or not .01<=t[0]<=100 or abs(t[1])>360 for t in targets)):
        raise ValueError('invalid repeated Splop grip placements')


def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('one Splop grip command required')
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if list(doc.Objects)or doc.UndoRecordingIsActive:raise ValueError('Splop grips require an empty owned document at idle')
    vp=doc.Views.ActiveView.ActiveViewport;saved_plane=vp.GetConstructionPlane();saved_tolerance=doc.ModelAbsoluteTolerance
    vp.SetConstructionPlane(G.Plane.WorldXY);doc.ModelAbsoluteTolerance=1e-6
    target=G.PlaneSurface(G.Plane(G.Point3d(0.,0.,5.),G.Vector3d.ZAxis),G.Interval(-20.,20.),G.Interval(-20.,20.)).ToNurbsSurface() if op['surface']=='Plane' else G.Cylinder(G.Circle(G.Plane.WorldXY,20.),20.).ToNurbsSurface()
    uv=[target.Domain(0).ParameterAt(.17),target.Domain(1).ParameterAt(.61)]
    ok,frame=target.FrameAt(*uv)
    if not ok:raise ValueError('Splop grip target frame unavailable')
    geometry=source.create_source(op['source'],host);guide=owner=peer=None
    def snapshot():
        gs=[g.Index for g in doc.Groups if not g.IsDeleted]
        rows=[]
        for o in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            if o.Id==guide:continue
            g=o.Geometry
            row=dict(role='source'if o.Id==owner else 'point'if o.Id==peer else'output',selected=bool(o.IsSelected(False)),name=o.Attributes.Name,
                     groups=[gs.index(i)for i in(o.GetGroupList()or[])],grips_on=bool(o.GripsOn),
                     grips=[dict(index=int(p.Index),point=host['_xyz'](p.CurrentLocation),selected=bool(p.IsSelected(False)))for p in(o.GetGrips()or[])],kind=g.GetType().Name)
            if isinstance(g,G.Curve):row['curve']=host['_nurbs_curve_definition'](g)
            elif isinstance(g,G.Brep):row['surface']=host['_nurbs_surface_definition'](g.Surfaces[0])
            elif isinstance(g,G.Mesh):row['mesh']=host['_polygon_mesh_value'](g)
            elif isinstance(g,G.Point):row['point']=host['_xyz'](g.Location)
            else:raise ValueError('unexpected Splop grip owner')
            rows.append(row)
        return dict(objects=rows,group_count=len(gs),guide_selected=bool(doc.Objects.FindId(guide).IsSelected(False)))
    point=lambda p:'w'+','.join('%.17g'%x for x in host['_xyz'](p))
    try:
        record=doc.BeginUndoRecord('Owned Splop grip sources')
        try:
            guide=doc.Objects.AddSurface(target)
            attributes=R.DocObjects.ObjectAttributes();attributes.Name='grip source'
            owner=doc.Objects.Add(geometry,attributes)
            if op.get('point'):peer=doc.Objects.AddPoint(G.Point3d(8.,1.,1.))
            if op.get('grouped'):doc.Groups.Add('OwnedSplopGrip',[owner]+([peer]if peer else[]))
        finally:doc.EndUndoRecord(record)
        obj=doc.Objects.FindId(owner)
        if op.get('edit'):
            doc.Objects.Select(owner);R.RhinoApp.RunScript('_EditPtOn',False);doc.Objects.UnselectAll();obj=doc.Objects.FindId(owner)
        else:obj.GripsOn=True
        grips=obj.GetGrips()or[]
        if not grips or any(i>=len(grips)for i in op['selected']):raise ValueError('Splop grip index unavailable')
        for i in op['selected']:grips[i].Select(True)
        if op.get('parent'):doc.Objects.Select(owner)
        if peer:doc.Objects.Select(peer)
        doc.ClearUndoRecords(True);before=snapshot()
        flags=' '.join('_'+k+'=_'+('Yes'if op[k.lower()]else'No')for k in ('Copy','Rigid','Flip'))
        macro='_Splop w0,0,0 2 '+"'_-SelID "+str(guide)+' '+flags
        for radius,angle in [(4.,0.)]+op.get('targets',[]):
            a=math.radians(angle);direction=frame.XAxis*(radius*math.cos(a))+frame.YAxis*(radius*math.sin(a))
            macro+=' '+point(frame.Origin)+' '+point(frame.Origin+direction)
        if op['copy']:macro+=' _Cancel'if op.get('finish')=='Cancel'else' _Enter _Cancel'
        marker='Owned Splop grips '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker);host['_record_progress'](op['id']+' '+macro)
        success,after,events=observe_command(R.Commands.Command,'Splop',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        after_script=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        undo_history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()[len(history):]
        return dict(success=success,before=before,after=after,after_script=after_script,undo=undo,redo=redo,history=history,undo_history=undo_history,events=events,macro=macro,surface=host['_nurbs_surface_definition'](target),uv=uv,target=host['_xyz'](frame.Origin),direction=host['_xyz'](frame.XAxis)),0
    finally:
        for o in list(doc.Objects):
            if o.GripsOn:o.GripsOn=False
            doc.Objects.Delete(o.Id,True)
        for g in list(doc.Groups):
            if not g.IsDeleted:doc.Groups.Delete(g.Index)
        geometry.Dispose();target.Dispose();vp.SetConstructionPlane(saved_plane);doc.ModelAbsoluteTolerance=saved_tolerance
