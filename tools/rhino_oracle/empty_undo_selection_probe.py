"""Direct public empty Undo selection with and without an available redo."""
import re


def validate(op):
    if (not isinstance(op,dict)or set(op)!={'op','id','kind','redo'}or op.get('op')!='empty_undo_selection'
        or not isinstance(op.get('id'),str)or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id'])is None
        or op['kind']not in('curve','point')or type(op['redo'])is not bool):raise ValueError('invalid empty Undo recipe')


def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('Undo selection capture requires one iteration')
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if list(doc.Objects)or R.Commands.Command.InCommand():raise ValueError('Undo capture requires an empty owned document')
    key=marker=S.Guid.Empty
    def snapshot():return dict(selected=bool(doc.Objects.FindId(key).IsSelected(False)),marker=doc.Objects.FindId(marker)is not None)
    try:
        key=doc.Objects.AddPoint(G.Point3d(0.,0.,0.))if op['kind']=='point'else doc.Objects.AddLine(G.Point3d(0.,0.,0.),G.Point3d(5.,0.,0.))
        doc.ClearUndoRecords(True);doc.ClearRedoRecords()
        serial=doc.BeginUndoRecord('Redo witness')
        try:marker=doc.Objects.AddPoint(G.Point3d(30.,30.,30.))
        finally:doc.EndUndoRecord(serial)
        R.RhinoApp.RunScript('_Undo',False)
        if not op['redo']:doc.ClearRedoRecords()
        doc.Objects.Select(key,True);before=snapshot();steps=[]
        for command in('Undo','Redo'):
            initial=snapshot();_,after,events=observe_command(R.Commands.Command,command,lambda:R.RhinoApp.RunScript('_'+command,False),snapshot,lambda:[],True)
            steps.append(dict(command=command,before=initial,after=after,after_script=snapshot(),results=[e['result']for e in events if e['name']==command]))
        return dict(before=before,steps=steps),0
    finally:
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True);doc.ClearRedoRecords()
