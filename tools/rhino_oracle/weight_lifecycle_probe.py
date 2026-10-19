"""Public grip-weight controllers before and after reopening their displays."""
import re


def validate(op):
    if (not isinstance(op,dict) or set(op)!={'op','id','kind','seed','preset'} or op.get('op')!='grip_weight_lifecycle'
            or not isinstance(op.get('id'),str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id'])is None
            or op.get('kind')not in ('line','weighted','polycurve','polycurve_curved')
            or op.get('seed')not in ('first','middle','ends') or type(op.get('preset'))is not bool
            or op['preset'] and op['kind']!='polycurve'):
        raise ValueError('invalid Weight lifecycle recipe')


def run(op,host):
    validate(op)
    from delete_grips_probe import create_source,picks
    from join_probe import observe_command
    from weight_dialog import watch
    R=host['Rhino'];doc=R.RhinoDoc.ActiveDoc;key=None
    if list(doc.Objects)or R.Commands.Command.InCommand():raise ValueError('Weight lifecycle requires an owned idle document')
    def snapshot():
        obj=doc.Objects.FindId(key);g=obj.Geometry
        row=dict(kind=g.GetType().Name,curve=host['_nurbs_curve_definition'](g),selected=bool(obj.IsSelected(False)),
            grips=[dict(index=int(p.Index),point=host['_xyz'](p.CurrentLocation),weight=float(p.Weight),selected=bool(p.IsSelected(False)))for p in(obj.GetGrips()or[])])
        if isinstance(g,R.Geometry.PolyCurve):
            pieces=list(g.DuplicateSegments())
            try:row['segments']=[dict(kind=s.GetType().Name,curve=host['_nurbs_curve_definition'](s))for s in pieces]
            finally:
                for s in pieces:s.Dispose()
        return row
    def execute(command):
        host['_record_progress'](op['id']+' '+command)
        before=snapshot();state=None
        if command=='Weight':
            timer,state=watch(host,[2.],False)
            try:_,after,events=observe_command(R.Commands.Command,command,lambda:R.RhinoApp.RunScript('_-Weight',False),snapshot,lambda:[],True)
            finally:timer.Stop()
            if state['errors']:raise ValueError(repr(state))
        else:_,after,events=observe_command(R.Commands.Command,command,lambda:R.RhinoApp.RunScript('_'+command,False),snapshot,lambda:[],True)
        outcomes=[e['result']for e in events if e['name']==command]
        if len(outcomes)!=1:raise ValueError('missing Weight lifecycle completion')
        return dict(command=command,before=before,after=after,after_script=snapshot(),result=outcomes[0],default=state['default']if state else None)
    try:
        source=create_source(op['kind'],host)
        if op['preset']:
            pieces=list(source.DuplicateSegments());replacement=R.Geometry.PolyCurve()
            try:
                for i,s in enumerate(pieces):
                    n=s.ToNurbsCurve()
                    try:
                        if i==0:n.Points.SetPoint(0,n.Points[0].Location,2.)
                        if i==len(pieces)-1:n.Points.SetPoint(n.Points.Count-1,n.Points[n.Points.Count-1].Location,2.)
                        if not replacement.Append(n):raise ValueError('cannot preset lifecycle leaf')
                    finally:n.Dispose()
            finally:
                source.Dispose()
                for s in pieces:s.Dispose()
            source=replacement
        try:key=doc.Objects.AddCurve(source)
        finally:source.Dispose()
        doc.Objects.FindId(key).GripsOn=True;doc.ClearUndoRecords(True);doc.ClearRedoRecords()
        def select():
            grips=list(doc.Objects.FindId(key).GetGrips());selected=picks(op['seed'],len(grips))
            for g in grips:g.Select(int(g.Index)in selected)
        select();before=snapshot();steps=[execute('Weight'),execute('PointsOff')]
        doc.Objects.Select(key,True);steps.append(execute('PointsOn'));select()
        steps.extend([execute('Weight'),execute('Undo'),execute('Redo')])
        return dict(before=before,steps=steps),0
    finally:
        for obj in list(doc.Objects):
            if obj.GripsOn:obj.GripsOn=False
            doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True);doc.ClearRedoRecords()
