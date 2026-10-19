"""Public history and redo witness for unchanged InsertEditPoint control nets."""
import re


def validate(op):
    if (not isinstance(op,dict) or set(op)!={'op','id','station','cancel','preselect','geometry_text'}
            or op.get('op')!='insert_edit_point_history' or not isinstance(op.get('id'),str)
            or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id'])is None
            or op['station']not in('empty','existing','new')
            or any(type(op.get(k))is not bool for k in('cancel','preselect','geometry_text'))):
        raise ValueError('invalid edit-point history recipe')


def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('history capture requires one iteration')
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if list(doc.Objects)or R.Commands.Command.InCommand():raise ValueError('history needs an owned idle document')
    key=marker=S.Guid.Empty
    def snapshot():
        obj=doc.Objects.FindId(key)
        return dict(same_id=obj.Id==key,selected=bool(obj.IsSelected(False)),name=obj.Attributes.Name,
            grips_on=bool(obj.GripsOn),geometry_text=obj.Geometry.GetUserString('GeometryCode'),
            curve=host['_nurbs_curve_definition'](obj.Geometry),marker=doc.Objects.FindId(marker)is not None)
    try:
        c=G.NurbsCurve.Create(False,3,S.Array[G.Point3d]([G.Point3d(*p)for p in [[0.,0.,0.],[1.,3.,0.],[3.,-1.,0.],[5.,0.,0.]]]))
        try:
            if op['geometry_text']:c.SetUserString('GeometryCode','source')
            a=R.DocObjects.ObjectAttributes();a.Name='source'
            try:key=doc.Objects.AddCurve(c,a)
            finally:a.Dispose()
            target=c.PointAt(1./3. if op['station']=='existing'else .35)
        finally:c.Dispose()
        doc.ClearUndoRecords(True);doc.ClearRedoRecords()
        serial=doc.BeginUndoRecord('Redo witness')
        try:marker=doc.Objects.AddPoint(G.Point3d(30.,30.,30.))
        finally:doc.EndUndoRecord(serial)
        R.RhinoApp.RunScript('_Undo',False)
        if op['preselect']:doc.Objects.Select(key,True)
        before=snapshot();steps=[]
        for command in ('InsertEditPoint','Undo','Redo'):
            host['_record_progress'](op['id']+' '+command)
            initial=snapshot()
            if command=='InsertEditPoint':
                macro='_InsertEditPoint '+(''if op['preselect']else'_SelID '+str(key)+' ')
                if op['station']!='empty':macro+='w'+','.join('%.17g'%v for v in host['_xyz'](target))+' '
                macro+='!'if op['cancel']else'_Enter'
            else:macro='_'+command
            _,after,events=observe_command(R.Commands.Command,command,lambda:R.RhinoApp.RunScript(macro,False),snapshot,lambda:[],True)
            outcomes=[e['result']for e in events if e['name']==command]
            if len(outcomes)!=1:raise ValueError('missing history command completion')
            steps.append(dict(command=command,before=initial,after=after,after_script=snapshot(),result=outcomes[0]))
        return dict(before=before,steps=steps),0
    finally:
        for obj in list(doc.Objects):
            if obj.GripsOn:obj.GripsOn=False
            doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True);doc.ClearRedoRecords()
