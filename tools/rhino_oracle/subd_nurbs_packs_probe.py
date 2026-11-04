"""Owned ToNURBS face-pack actions on a public SDK quad sphere."""
if __package__:
    from .join_probe import observe_command
else:
    from join_probe import observe_command

ACTIONS={
    'force_cancel':'_SubDOptions _ForceUpdateFacePacks !',
    'force_accept':'_SubDOptions _ForceUpdateFacePacks _Enter _Enter',
    'show_cancel':'_SubDOptions _ShowFacePacks=_Yes !',
    'mixed':'_SubDOptions _ForceUpdateFacePacks=_Yes _ShowFacePacks=_Yes _Enter _Enter',
    'force_off':'_SubDOptions _ForceUpdateFacePacks=_Yes _ForceUpdateFacePacks=_No _Enter _Enter',
    'force_twice':'_SubDOptions _ForceUpdateFacePacks _ForceUpdateFacePacks _Enter _Enter',
    'force_unpacked':'_SubDOptions _ForceUpdateFacePacks=_Yes _Faces=_Unpacked _Enter _Enter',
    'show_accept':'_SubDOptions _Faces=_Packed _ShowFacePacks=_Yes _Enter _Enter',
    'baseline_after_flags':'_SubDOptions _Enter _Enter',
    'force_options_cancel':'_SubDOptions _ForceUpdateFacePacks=_Yes _Enter !',
    'post_force_cancel':'_SubDOptions _ForceUpdateFacePacks=_Yes !',
}
def discovery_cases():return [dict(op='subd_nurbs_packs',id=k,case=k) for k in ('force_cancel','force_accept','show_cancel')]
def cases():
    rows=[]
    for delete in (False,True):
        for post in (False,True):
            rows.append(dict(op='subd_nurbs_packs',id='mixed_'+str(delete)+'_'+str(post),case='mixed',delete=delete,postselect=post))
    for key in ('force_off','force_twice','force_unpacked','show_accept','baseline_after_flags','force_options_cancel','post_force_cancel'):
        rows.append(dict(op='subd_nurbs_packs',id=key,case=key))
    return rows

def validate(op):
    if (not isinstance(op,dict) or op not in discovery_cases()+cases() or
            any(type(op[k]) is not bool for k in ('delete','postselect') if k in op)):raise ValueError('fixed owned ToNURBS pack action required')

def run(op,host,iterations,tolerance):
    validate(op);R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or doc.UndoRecordingIsActive or R.Commands.Command.InCommand():
        raise ValueError('empty private idle document required')
    keys=[];serials=[]
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g=obj.Geometry;a=obj.Attributes
            row=dict(source=keys.index(obj.Id) if obj.Id in keys else None,kind=g.GetType().Name,
                selected=bool(obj.IsSelected(False)),crc=int(g.DataCRC(0)),name=a.Name,
                groups=[int(i) for i in (a.GetGroupList() or [])],same_serial=obj.RuntimeSerialNumber in serials)
            if isinstance(g,G.SubD):
                model=R.FileIO.File3dm()
                try:model.Objects.AddSubD(g);row['archive']=[int(b) for b in model.ToByteArray()]
                finally:model.Dispose()
            if isinstance(g,G.Brep):row['brep']=host['_interchange_brep_record'](g)
            rows.append(row)
        return rows
    try:
        s=G.SubD.CreateQuadSphere(G.Sphere(G.Point3d(1,2,3),4),G.SubDComponentLocation.ControlNet,S.UInt32(1))
        if s is None:raise ValueError('cannot create owned quad sphere')
        try:keys.append(doc.Objects.AddSubD(s))
        finally:s.Dispose()
        if keys[0]==S.Guid.Empty:raise ValueError('cannot insert owned quad sphere')
        if op['case']=='mixed':
            keys.append(doc.Objects.AddPoint(G.Point3d(20,0,0)))
            keys.append(doc.Objects.AddLine(G.Point3d(30,0,0),G.Point3d(34,1,0)))
        for i,key in enumerate(keys):
            obj=doc.Objects.FindId(key);serials.append(obj.RuntimeSerialNumber)
            a=obj.Attributes.Duplicate();a.Name='Input '+str(i)
            try:
                if not doc.Objects.ModifyAttributes(key,a,True):raise ValueError('cannot name source')
            finally:a.Dispose()
        post=op.get('postselect') or op['case']=='post_force_cancel'
        if not post:
            for key in keys:doc.Objects.Select(key)
        doc.ClearUndoRecords(True)
        before=snapshot();marker='Owned ToNURBS packs '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        macro='_-ToNURBS'
        if post:macro+=' '+' '.join('_SelID '+str(keys[i]) for i in reversed(range(len(keys))) if i!=1)+' _Enter'
        macro+=' _DeleteInputObjects=_'+('Yes' if op.get('delete') else 'No')+' '+ACTIONS[op['case']]
        success,after,events=observe_command(R.Commands.Command,'ToNURBS',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        if R.Commands.Command.InCommand():raise ValueError('face-pack macro left a command active')
        settled=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(macro=macro,before=before,after=after,settled=settled,undo=undo,redo=redo,history=history,
            success=success,results=[e['result'] for e in events if e['name']=='ToNURBS']),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
