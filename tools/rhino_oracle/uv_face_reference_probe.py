"""Read-only face references for public UV commands on one six-face box."""
import re
CASES=tuple('create_'+str(i) for i in range(6))+tuple('apply_'+str(i) for i in range(6))+('apply_top','apply_front')
def validate_request(q):
    if type(q.get('protocol_version')) is not int or q['protocol_version']!=1 or type(q.get('iterations',1)) is not int or q.get('iterations',1)!=1 or not isinstance(q.get('operations'),list) or not 1<=len(q['operations'])<=16:
        raise ValueError('UV face references require bounded protocol 1 recipes')
    seen=set()
    for op in q['operations']:
        if not isinstance(op,dict) or set(op)!= {'op','id','case'} or op['op']!='uv_face_reference_command' or not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None or op['id'] in seen or op.get('case') not in CASES:
            raise ValueError('invalid UV face reference recipe')
        seen.add(op['id'])
def request():return dict(protocol_version=1,iterations=1,operations=[dict(op='uv_face_reference_command',id='uv_face_'+c,case=c) for c in CASES])
def run(op,host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino,System=host['Rhino'],host['System'];G=Rhino.Geometry;doc=Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand():raise ValueError('UV face references require idle execution')
    if list(doc.Objects):raise ValueError('UV face references require an empty owned document')
    from join_probe import observe_command
    owned=[];ids=[];saved_tolerance=doc.ModelAbsoluteTolerance;doc.ModelAbsoluteTolerance=1e-6
    def keep(g):
        if g is None or not g.IsValid:raise ValueError('invalid UV face source')
        owned.append(g);return g
    def surfaces(b):
        result=[]
        for face in b.Faces:
            n=face.UnderlyingSurface().ToNurbsSurface()
            try:result.append(host['_nurbs_surface_definition'](n))
            finally:n.Dispose()
        return result
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g=obj.Geometry;row=dict(source=ids.index(obj.Id) if obj.Id in ids else None,selected=bool(obj.IsSelected(False)))
            if isinstance(g,G.Brep):row.update(kind='brep',surfaces=surfaces(g),face_count=g.Faces.Count)
            else:
                n=g.ToNurbsCurve()
                try:row.update(kind='curve',definition=host['_nurbs_curve_definition'](n),samples=[host['_xyz'](g.PointAt(g.Domain.ParameterAt(i/32.))) for i in range(33)])
                finally:n.Dispose()
            rows.append(row)
        return rows
    try:
        box=keep(G.BoundingBox(G.Point3d(0.,0.,0.),G.Point3d(4.,6.,2.)).ToBrep())
        ids.append(doc.Objects.AddBrep(box));face_centers=[]
        for f in box.Faces:face_centers.append(host['_xyz'](f.PointAt(f.Domain(0).Mid,f.Domain(1).Mid)))
        if op['case'].startswith('create_'):
            face=int(op['case'].split('_')[1]);command='CreateUVCrv'
            if not doc.Objects.FindId(ids[0]).SelectSubObject(G.ComponentIndex(G.ComponentIndexType.BrepFace,face),True,True,False):raise ValueError('UV face preselection failed')
            macro='_CreateUVCrv _Enter'
        else:
            preselected=op['case'].split('_')[1].isdigit()
            point=(2.,0.,1.) if op['case']=='apply_front' else (2.,3.,2.)
            face=int(op['case'].split('_')[1]) if preselected else min(range(len(face_centers)),key=lambda i:sum((a-b)**2 for a,b in zip(point,face_centers[i])))
            command='ApplyCrv'
            curve=keep(G.LineCurve(G.Point3d(0.,0.,0.),G.Point3d(1.,1.,0.)))
            ids.append(doc.Objects.AddCurve(curve));doc.Objects.Select(ids[1])
            if op['case']=='apply_front':Rhino.RhinoApp.RunScript('_SetView _World _Front',False)
            else:Rhino.RhinoApp.RunScript('_SetView _World _Top',False)
            Rhino.RhinoApp.RunScript('_Zoom _Extents',False)
            if preselected:
                if not doc.Objects.FindId(ids[0]).SelectSubObject(G.ComponentIndex(G.ComponentIndexType.BrepFace,face),True,True,False):raise ValueError('ApplyCrv face preselection failed')
                macro='_ApplyCrv _Enter'
            else:macro='_ApplyCrv '+','.join(str(x) for x in point)+' _Enter'
        doc.ClearUndoRecords(True);before=snapshot()
        marker='Viboceros UV face '+str(System.Guid.NewGuid());Rhino.RhinoApp.WriteLine(marker)
        success,after,events=observe_command(Rhino.Commands.Command,command,lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        if Rhino.Commands.Command.InCommand():raise ValueError('UV face command remains active')
        history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        Rhino.RhinoApp.RunScript('_Undo',False);undo=snapshot()
        Rhino.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(command=command,face=face,box_bounds=[[0.,4.],[0.,6.],[0.,2.]],face_centers=face_centers,before=before,after=after,success=success,events=events,history=history,undo=undo,redo=redo),0
    finally:
        if Rhino.Commands.Command.InCommand():Rhino.RhinoApp.RunScript('!',False)
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ModelAbsoluteTolerance=saved_tolerance
        for g in reversed(owned):g.Dispose()
