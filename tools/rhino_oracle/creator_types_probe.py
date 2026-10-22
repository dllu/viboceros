"""Fixed public creator commands and their resulting geometry representations."""
CASES = {
    'creator_box': dict(command='Box', macro='_Box 0,0,0 4,3,0 2'),
    'creator_cylinder': dict(command='Cylinder', macro='_Cylinder 0,0,0 2 3'),
}
for profile in ['rectangle','circle','open_kink']:
    for solid in [False,True]:
        CASES['creator_extrude_'+profile+'_'+str(int(solid))] = dict(command='ExtrudeCrv',profile=profile,
            macro='_-ExtrudeCrv _Output=_Surface _Solid=_'+('Yes' if solid else 'No')+' _BothSides=_No _DeleteInput=_No 2')


def request():
    return dict(protocol_version=1, iterations=1, tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6),
        operations=[dict(op='creator_types',id=k,case=k)for k in CASES])


def validate(operation):
    try:
        valid=isinstance(operation,dict) and set(operation)=={'op','id','case'} and operation['op']=='creator_types' and operation['case'] in CASES and operation['id']==operation['case']
    except TypeError:
        valid=False
    if not valid:raise ValueError('fixed creator representation recipe required')


def run(operation,host,iterations,tolerance):
    validate(operation)
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or doc.UndoRecordingIsActive:raise ValueError('empty owned idle document required')
    case=CASES[operation['case']];source=[]
    def snapshot():
        result=[]
        for obj in doc.Objects:
            g=obj.Geometry
            row=dict(source=obj.Id in source,type=str(g.GetType().Name),object_type=str(g.ObjectType))
            if isinstance(g,G.Extrusion):row.update(profile_count=int(g.ProfileCount),cap_count=int(g.CapCount))
            if isinstance(g,G.Brep):row['faces']=int(g.Faces.Count)
            result.append(row)
        return result
    try:
        if 'profile' in case:
            record=doc.BeginUndoRecord('Owned creator source')
            try:
                profile=case['profile']
                curve=G.Rectangle3d(G.Plane.WorldXY,4.,3.).ToNurbsCurve() if profile=='rectangle' else G.Circle(G.Plane.WorldXY,2.).ToNurbsCurve() if profile=='circle' else G.PolylineCurve([G.Point3d(0.,0.,0.),G.Point3d(1.,1.,0.),G.Point3d(3.,0.,0.)])
                try:source.append(doc.Objects.AddCurve(curve))
                finally:curve.Dispose()
                doc.Objects.Select(source[0])
            finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True)
        marker='Owned creator '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        success,created,events=observe_command(R.Commands.Command,case['command'],lambda:R.RhinoApp.RunScript(case['macro']+' _Cancel _Cancel _Cancel',True),snapshot,lambda:[],True)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        outputs=[obj for obj in doc.Objects if obj.Id not in source]
        if not success or len(outputs)!=1 or 'Unknown command:'in history:raise ValueError('creator failed: '+str(created)+' '+history)
        doc.Objects.UnselectAll();doc.Objects.Select(outputs[0].Id)
        converted,after,conversion_events=observe_command(R.Commands.Command,'ToSubD',lambda:R.RhinoApp.RunScript('_-ToSubD _Enter _Cancel _Cancel _Cancel',True),snapshot,lambda:[],True)
        return dict(input=case,success=success,created=created,events=events,history=history,
            tosubd_success=converted,after=after,conversion_events=conversion_events),0
    finally:
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
