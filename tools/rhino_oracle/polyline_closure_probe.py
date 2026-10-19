"""Public closure, selection, grips and point extraction in an owned document."""
import math
import re

FORMS=('polyline','nurbs','weighted_nurbs','unit_rational_nurbs','polycurve','nested')

def validate(op):
    if (not isinstance(op,dict)or set(op)!={'op','id','cases','artifact_path'}or op['op']!='polyline_closure'
        or not isinstance(op['id'],str)or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id'])is None
        or not isinstance(op['artifact_path'],str)or not op['artifact_path'].endswith('.3dm')
        or not isinstance(op['cases'],list)or not 1<=len(op['cases'])<=256):raise ValueError('invalid closure matrix')
    names=set()
    for c in op['cases']:
        if (not isinstance(c,dict)or not {'id','points','form'}<=set(c)or set(c)-{'id','points','form','domain','extract','degree','move_grip','grip_index','move_vector'}
            or not isinstance(c['id'],str)or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',c['id'])is None
            or c['id']in names or c['form']not in FORMS or type(c.get('extract',False))is not bool or type(c.get('move_grip',False))is not bool):raise ValueError('invalid closure source')
        names.add(c['id']);p=c['points']
        if (not isinstance(p,list)or not 2<=len(p)<=64 or any(not isinstance(q,list)or len(q)!=3 or any(type(x)not in(int,float)or math.isnan(x)or math.isinf(x)or abs(x)>1e13 for x in q)for q in p)
            or any(a==b for a,b in zip(p,p[1:]))):raise ValueError('invalid closure vertices')
        degree=c.get('degree',1)
        if type(degree)is not int or not 1<=degree<=3 or degree>=len(p) or ('degree'in c and c['form']not in('nurbs','weighted_nurbs','unit_rational_nurbs')):raise ValueError('invalid source degree')
        if type(c.get('grip_index',0))is not int or not 0<=c.get('grip_index',0)<len(p):raise ValueError('invalid source grip')
        vector=c.get('move_vector',[1.,2.,3.])
        if not isinstance(vector,list)or len(vector)!=3 or any(type(x)not in(int,float)or math.isnan(x)or math.isinf(x)or abs(x)>1e3 for x in vector):raise ValueError('invalid grip move')
        if 'domain'in c:
            d=c['domain']
            if not isinstance(d,list)or len(d)!=2 or any(type(x)not in(int,float)or math.isnan(x)or math.isinf(x)for x in d)or not d[0]<d[1]:raise ValueError('invalid closure domain')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('closure capture requires one iteration')
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    from join_probe import observe_command
    if R.Commands.Command.InCommand()or list(doc.Objects):raise ValueError('closure capture requires an owned empty idle document')
    owned=[];attributes=[];ids=[];rows=[]
    def keep(c):
        if c is None or not c.IsValid:raise ValueError('invalid native closure source')
        owned.append(c);return c
    def record(c):
        if isinstance(c,G.PolylineCurve):return dict(type='polyline',vertices=[host['_xyz'](c.Point(i))for i in range(c.PointCount)],parameters=host['_nurbs_curve_definition'](c)['knots'][1:-1])
        if isinstance(c,G.LineCurve):return dict(type='line',start=host['_xyz'](c.PointAtStart),end=host['_xyz'](c.PointAtEnd),domain=[float(c.Domain.T0),float(c.Domain.T1)])
        if isinstance(c,G.PolyCurve):return dict(type='polycurve',segments=[record(c.SegmentCurve(i))for i in range(c.SegmentCount)],parameters=[float(c.SegmentDomain(i).T0)for i in range(c.SegmentCount)]+[float(c.Domain.T1)])
        return dict(type='nurbs',definition=host['_nurbs_curve_definition'](c),rational=bool(c.IsRational))
    def selected():return [str(o.Id)for o in doc.Objects if o.IsSelected(False)]
    def command(name,macro,snapshot):
        success,value,events=observe_command(R.Commands.Command,name,lambda:R.RhinoApp.RunScript(macro,True),snapshot,selected,True)
        return dict(success=success,value=value,events=[dict(name=e['name'],result=e['result'])for e in events],macro=macro)
    try:
        for case in op['cases']:
            host['_record_progress']('closure source '+case['id'])
            p=case['points'];pts=[G.Point3d(*q)for q in p];form=case['form'];n=len(p)
            if form=='polyline':source=G.PolylineCurve(S.Array[G.Point3d](pts));owned.append(source)
            elif form in('nurbs','weighted_nurbs','unit_rational_nurbs'):
                degree=case.get('degree',1);last=float(n-degree)
                knots=[0.]*(degree+1)+[float(i)for i in range(1,n-degree)]+[last]*(degree+1)
                source=keep(host['_nurbs_curve_from_definition'](dict(degree=degree,knots=knots,control_points=[dict(point=q,weight=1.+i%3 if form=='weighted_nurbs'else 1.)for i,q in enumerate(p)])))
                if form=='nurbs':source.Points.MakeNonRational()
            else:
                parts=[G.LineCurve(a,b)for a,b in zip(pts,pts[1:])];owned.extend(parts)
                if any(not c.IsValid for c in parts):
                    ids.append(None);rows.append(dict(id=None,valid=False,recipe=case,closed=None,geometry=None,grips=None,construction_failure='invalid native line segment'))
                    continue
                source=G.PolyCurve();owned.append(source)
                if form=='nested'and len(parts)>2:
                    child=G.PolyCurve();owned.append(child)
                    for part in parts[:2]:
                        if not child.Append(part):raise ValueError('nested source append failed')
                    parts=[child]+parts[2:]
                for part in parts:
                    if not source.Append(part):raise ValueError('source append failed')
            if 'domain'in case:source.Domain=G.Interval(*case['domain'])
            if not source.IsValid:
                ids.append(None);rows.append(dict(id=None,valid=False,recipe=case,closed=bool(source.IsClosed),geometry=record(source),grips=None,sdk_polyline_closed=None))
                continue
            source.SetUserString('ClosureSource',case['id'])
            a=R.DocObjects.ObjectAttributes();attributes.append(a);a.Name=case['id'];a.SetUserString('ClosureSource',case['id'])
            key=doc.Objects.AddCurve(source,a)
            if key==S.Guid.Empty:raise ValueError('closure source insertion failed')
            ids.append(key);obj=doc.Objects.FindId(key);obj.GripsOn=True
            grips=[host['_xyz'](g.CurrentLocation)for g in(obj.GetGrips()or[])];obj.GripsOn=False
            ok,pl=source.TryGetPolyline()
            rows.append(dict(id=str(key),valid=True,recipe=case,closed=bool(source.IsClosed),grips=grips,geometry=record(source),
                sdk_polyline_closed=bool(pl.IsClosed)if ok else None))
        path=op['artifact_path'];path='Z:'+path.replace('/','\\')if path.startswith('/')else path
        options=R.FileIO.FileWriteOptions();options.FileVersion=8;options.SuppressDialogBoxes=True;options.SuppressAllInput=True;options.WriteUserData=True;options.UpdateDocumentPath=False
        try:
            if not doc.Write3dmFile(path,options):raise ValueError('closure source save failed')
        finally:options.Dispose()
        doc.ClearUndoRecords(True);queries={}
        for name in('SelClosedCrv','SelOpenCrv'):
            doc.Objects.UnselectAll();queries[name]=command(name,'_'+name,selected)
        doc.Objects.UnselectAll()
        for key,row in zip(ids,rows):
            if key is None or not row['recipe'].get('move_grip'):continue
            host['_record_progress']('closure grip move '+row['recipe']['id'])
            doc.Objects.UnselectAll();obj=doc.Objects.FindId(key);obj.GripsOn=True
            obj.GetGrips()[row['recipe'].get('grip_index',0)].Select(True)
            vector=row['recipe'].get('move_vector',[1.,2.,3.]);macro='_Move w0,0,0 w%.17g,%.17g,%.17g'%tuple(vector)
            row['move_grip']=command('Move',macro,lambda:record(doc.Objects.FindId(key).Geometry))
            R.RhinoApp.RunScript('_Undo',False);doc.Objects.FindId(key).GripsOn=False
            doc.ClearUndoRecords(True)
        for key,row in zip(ids,rows):
            if key is None or not row['recipe'].get('extract'):continue
            host['_record_progress']('closure extract '+row['recipe']['id'])
            doc.Objects.UnselectAll();doc.Objects.Select(key);marker='Closure ExtractPt '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
            row['extract']=command('ExtractPt','_ExtractPt',lambda:[dict(point=host['_xyz'](o.Geometry.Location),selected=bool(o.IsSelected(False)))for o in doc.Objects if isinstance(o.Geometry,G.Point)])
            row['extract']['history']=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
            for obj in list(doc.Objects):
                if isinstance(obj.Geometry,G.Point):doc.Objects.Delete(obj.Id,True)
        doc.Objects.UnselectAll()
        closed_ids=[key for key,row in zip(ids,rows)if key is not None and row['closed']]
        if closed_ids:
            host['_record_progress']('closure close with open witness')
            witness=keep(G.PolylineCurve(S.Array[G.Point3d]([G.Point3d(20.,0.,0.),G.Point3d(21.,0.,0.),G.Point3d(21.,1.,0.)])))
            witness_id=doc.Objects.AddCurve(witness)
            if witness_id==S.Guid.Empty:raise ValueError('close witness insertion failed')
            doc.Objects.Select(witness_id)
            for key in closed_ids:doc.Objects.Select(key)
            queries['CloseCrv']=command('CloseCrv','_CloseCrv',lambda:[dict(id=str(key),geometry=record(doc.Objects.FindId(key).Geometry))for key in ids if key is not None])
        return dict(rows=rows,queries=queries,document_tolerance=float(doc.ModelAbsoluteTolerance)),0
    finally:
        if R.Commands.Command.InCommand():R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):
            obj.GripsOn=False;doc.Objects.Delete(obj.Id,True)
        for a in reversed(attributes):a.Dispose()
        for g in reversed(owned):g.Dispose()
