"""Bounded, owned public Splop commands with explicit sphere directions."""
import math
import re
try:
    from . import twist_command_probe as geometry
except (ImportError, ValueError):
    import twist_command_probe as geometry


def validate(op):
    required = {'op','id','shape','surface','copy','rigid','flip','grouped','angle','radius'}
    optional = {'plane','reference_point','locked_radius','postselect','reverse','targets','finish'}
    if (not isinstance(op,dict) or set(op)-optional != required or op['op']!='splop_command' or
            not isinstance(op['id'],str) or re.match(r'\A[A-Za-z0-9_-]{1,80}\Z',op['id']) is None or
            op['shape'] not in geometry.SHAPES+('Cloud',) or op['surface'] not in ('Plane','Cylinder','Sphere') or
            any(type(op[k]) is not bool for k in ('copy','rigid','flip','grouped')) or
            op.get('plane','WorldXY') not in ('WorldXY','WorldZX','WorldYZ') or
            op.get('finish','Complete') not in ('Complete','Cancel')):
        raise ValueError('invalid bounded Splop command')
    def number(x): return type(x) in (int,float) and not math.isnan(x) and not math.isinf(x)
    if not number(op['angle']) or abs(op['angle'])>720 or not number(op['radius']) or not .01<=op['radius']<=100:
        raise ValueError('invalid Splop destination sphere')
    if any(type(op.get(k,False)) is not bool for k in ('reference_point','postselect','reverse')):
        raise ValueError('invalid Splop input flag')
    if 'locked_radius' in op and (not number(op['locked_radius']) or not .01<=op['locked_radius']<=100):
        raise ValueError('invalid Splop radius lock')
    if 'targets' in op and (not op['copy'] or not isinstance(op['targets'],list) or not 1<=len(op['targets'])<=3 or
            any(not isinstance(t,list) or len(t)!=2 or not all(number(x) for x in t) or not .01<=t[0]<=100 or abs(t[1])>720 for t in op['targets'])):
        raise ValueError('invalid repeated Splop spheres')


def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1: raise ValueError('one Splop command required')
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if list(doc.Objects): raise ValueError('Splop requires an empty owned document')
    if doc.UndoRecordingIsActive: raise ValueError('Splop command capture must run at idle, outside RunPythonScript undo recording')
    vp=doc.Views.ActiveView.ActiveViewport
    saved_plane=vp.GetConstructionPlane();saved_tolerance=doc.ModelAbsoluteTolerance
    plane=getattr(G.Plane,op.get('plane','WorldXY'))
    vp.SetConstructionPlane(plane)
    if op['surface']=='Plane':
        surface=G.PlaneSurface(G.Plane(G.Point3d(0.,0.,5.),G.Vector3d.ZAxis),G.Interval(-20.,20.),G.Interval(-20.,20.)).ToNurbsSurface()
    elif op['surface']=='Cylinder':
        surface=G.Cylinder(G.Circle(G.Plane.WorldXY,20.),20.).ToNurbsSurface()
    else:
        surface=G.Sphere(G.Point3d.Origin,20.).ToNurbsSurface()
    uv=[surface.Domain(0).ParameterAt(.17),surface.Domain(1).ParameterAt(.61)]
    ok,frame=surface.FrameAt(*uv)
    if not ok: raise ValueError('Splop target frame unavailable')
    ids=[];owned=[];guide=None
    def snapshot():
        groups=[g.Index for g in doc.Groups if not g.IsDeleted]
        return dict(objects=[dict(source=ids.index(o.Id) if o.Id in ids else None,
                   selected=bool(o.IsSelected(False)),name=o.Attributes.Name,
                   groups=[groups.index(g) for g in (o.Attributes.GetGroupList() or [])],
                   geometry=dict(type='PointCloud',points=[host['_xyz'](p.Location) for p in o.Geometry]) if isinstance(o.Geometry,G.PointCloud) else geometry.geometry_record(o.Geometry,host))
                   for o in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber) if o.Id!=guide],group_count=len(groups),
                   guide_selected=bool(doc.Objects.FindId(guide).IsSelected(False)))
    def guide_record():
        # Keep full definition inspection outside the Undo/Redo measurements.
        copy=doc.Objects.FindId(guide).Geometry.DuplicateBrep()
        try:return host['_interchange_brep_record'](copy,include_samples=False)
        finally:copy.Dispose()
    point=lambda p:'w'+','.join('%.17g'%x for x in p)
    try:
        doc.ModelAbsoluteTolerance=1e-6
        record=doc.BeginUndoRecord('Owned Splop sources')
        try:
            parent=surface.ToBrep()
            if op.get('reverse'):parent.Flip()
            guide=doc.Objects.AddBrep(parent);parent.Dispose()
            owned=[G.PointCloud([G.Point3d(*p) for p in [[0.,0.,0.],[1.,0.,0.],[0.,1.,0.],[0.,0.,1.],[2.,1.,.5]]])] if op['shape']=='Cloud' else geometry.source_geometry(op['shape'],host)
            for i,g in enumerate(owned):
                a=R.DocObjects.ObjectAttributes();a.Name='Splop source '+str(i)
                ids.append(doc.Objects.Add(g,a))
            if op['grouped']:doc.Groups.Add('OwnedSplop',ids)
            if not op.get('postselect'):
                for i in ids:doc.Objects.Select(i)
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True)
        before=snapshot()
        guide_before=guide_record()
        flags=' '.join('_'+k+'=_'+('Yes' if op[k.lower()] else 'No') for k in ('Copy','Rigid','Flip'))
        prefix='' if not op.get('postselect') else ' '.join('_SelID '+str(i) for i in ids)+' _Enter '
        reference=point(host['_xyz'](plane.Origin+plane.YAxis*2.)) if op.get('reference_point') else '2'
        macro='_Splop '+prefix+'w0,0,0 '+reference+' '+"'_-SelID "+str(guide)+' '+flags
        for radius,angle in [(op['radius'],op['angle'])]+op.get('targets',[]):
            radians=math.radians(angle)
            radial=frame.XAxis*(radius*math.cos(radians))+frame.YAxis*(radius*math.sin(radians))
            macro+=' '+point(host['_xyz'](frame.Origin))+' '
            if 'locked_radius' in op:macro+=repr(float(op['locked_radius']))+' '
            macro+=point(host['_xyz'](frame.Origin+radial))
        if op['copy']:macro+=' _Cancel' if op.get('finish')=='Cancel' else ' _Enter _Cancel'
        marker='Viboceros Splop '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        host['_record_progress'](op['id']+' '+macro)
        success,after,events=observe_command(R.Commands.Command,'Splop',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        after_script=snapshot()
        undo_status=R.RhinoApp.RunScript('_Undo',False);undo=snapshot()
        redo_status=R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        undo_history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()[len(history):]
        recording=dict(enabled=bool(doc.UndoRecordingEnabled),active=bool(doc.UndoRecordingIsActive))
        guide_unchanged=guide_before==guide_record()
        return dict(success=success,surface=host['_nurbs_surface_definition'](surface),uv=uv,frame=dict(origin=host['_xyz'](frame.Origin),x=host['_xyz'](frame.XAxis),y=host['_xyz'](frame.YAxis)),before=before,after=after,after_script=after_script,guide_unchanged=guide_unchanged,effective_reversed=bool(doc.Objects.FindId(guide).Geometry.Faces[0].OrientationIsReversed),undo=undo,redo=redo,undo_status=bool(undo_status),redo_status=bool(redo_status),undo_history=undo_history,recording=recording,events=events,history=history,macro=macro),0
    finally:
        for o in list(doc.Objects):doc.Objects.Delete(o.Id,True)
        for g in list(doc.Groups):
            if not g.IsDeleted:doc.Groups.Delete(g.Index)
        for g in owned:g.Dispose()
        surface.Dispose();vp.SetConstructionPlane(saved_plane);doc.ModelAbsoluteTolerance=saved_tolerance
