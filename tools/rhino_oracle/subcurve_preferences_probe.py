"""Closed SubCrv preference lifetime sequence in a fresh private profile."""
import re

STEPS=(
    dict(query=True),
    dict(copy=True,finish='Cancel'),dict(query=True),
    dict(copy=True,finish='Complete'),dict(query=True),
    dict(remember=False),dict(query=True),
    dict(copy=True,finish='Complete'),dict(remember=True),dict(query=True),
    dict(remember=False),dict(copy=True,finish='Cancel'),dict(remember=True),dict(query=True),
    dict(mode='MarkEnds',finish='Cancel'),dict(query=True),
    dict(mode='MarkEnds',finish='Complete'),dict(query=True),
    dict(mode='Shorten',midpoint=True,finish='Cancel'),dict(query=True),
    dict(midpoint=True,finish='Complete'),dict(query=True),
    dict(midpoint=False,mode='Shorten',copy=False,finish='Complete'),dict(query=True),
    dict(mode='MarkEnds',finish='Cancel'),dict(query=True),dict(finish='Complete'),dict(query=True),
    dict(remember=False),dict(copy=True,finish='Cancel'),dict(query=True),
    dict(copy=False,mode='Shorten',finish='Cancel'),dict(query=True),
    dict(midpoint=True,finish='Cancel'),dict(query=True),dict(midpoint=False,finish='Cancel'),dict(query=True))
def request():return dict(protocol_version=1,iterations=1,operations=[dict(op='subcurve_preferences',id='subcurve_preferences',case='lifetime')])
def validate_request(q):
    if (not isinstance(q,dict) or type(q.get('protocol_version')) is not int or q['protocol_version']!=1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1)!=1
            or not isinstance(q.get('operations'),list) or len(q['operations'])!=1):raise ValueError('SubCrv preferences require one closed protocol 1 workflow')
    op=q['operations'][0]
    if not isinstance(op,dict) or set(op)!={'op','id','case'} or op.get('op')!='subcurve_preferences' or op.get('case')!='lifetime' or not isinstance(op.get('id'),str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None:raise ValueError('invalid SubCrv preference workflow')
def run(op,host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino,System=host['Rhino'],host['System'];G=Rhino.Geometry;doc=Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand():raise ValueError('preferences require idle execution')
    if list(doc.Objects):raise ValueError('preferences require empty owned document')
    from join_probe import observe_command
    rows=[];owned=[];saved=doc.ModelAbsoluteTolerance;doc.ModelAbsoluteTolerance=1e-6
    try:
        for index,step in enumerate(STEPS):
            ids=[]
            def snapshot():
                result=[]
                for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
                    g=obj.Geometry;row=dict(source=ids.index(obj.Id) if obj.Id in ids else None,selected=bool(obj.IsSelected(False)))
                    if isinstance(g,G.Point):row.update(kind='point',point=host['_xyz'](g.Location))
                    else:
                        n=g.ToNurbsCurve()
                        try:row.update(kind='curve',definition=host['_nurbs_curve_definition'](n),samples=[host['_xyz'](g.PointAt(g.Domain.ParameterAt(i/16.))) for i in range(17)])
                        finally:n.Dispose()
                    result.append(row)
                return result
            if 'remember' in step:
                command='RememberCopyOptions';macro='_RememberCopyOptions _'+('Yes' if step['remember'] else 'No')
            else:
                source=G.LineCurve(G.Point3d(0.,0.,0.),G.Point3d(4.,6.,0.));owned.append(source);ids.append(doc.Objects.AddCurve(source));doc.Objects.Select(ids[0]);command='SubCrv';macro='_SubCrv'
                for key,option in [('copy','Copy'),('mode','Mode'),('midpoint','FromMidpoint')]:
                    if key in step:
                        value=('Yes' if step[key] else 'No') if type(step[key]) is bool else step[key]
                        macro+=' _'+option+'=_'+value
                macro+=' _Cancel' if step.get('query') or step.get('finish')=='Cancel' else ' 2,3,0 3,4.5,0 _Enter'
            doc.ClearUndoRecords(True);before=snapshot();marker='Viboceros SubCrv preferences '+str(System.Guid.NewGuid());Rhino.RhinoApp.WriteLine(marker)
            success,after,events=observe_command(Rhino.Commands.Command,command,lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
            active=bool(Rhino.Commands.Command.InCommand())
            if active:Rhino.RhinoApp.RunScript('!',False)
            history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
            choices={}
            for name in ('Copy','FromMidpoint','Mode'):
                matches=re.findall(name+r'=(Yes|No|Shorten|MarkEnds)',history)
                choices[name]=matches[-1] if matches else None
            rows.append(dict(index=index,step=step,macro=macro,before=before,after=after,success=success,command_active=active,history=history,choices=choices,events=events))
            doc.Objects.UnselectAll()
            for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        return dict(records=rows),0
    finally:
        if Rhino.Commands.Command.InCommand():Rhino.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ModelAbsoluteTolerance=saved
        for g in reversed(owned):g.Dispose()
