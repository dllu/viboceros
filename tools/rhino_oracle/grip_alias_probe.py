"""Closed public SDK witnesses for extraction and editable seam grips."""
import re

CASES = ('exact','roundoff','small','near','outside','open','smoothed_circle')
DELTAS = dict(exact=0.,roundoff=1e-16,small=1e-12,near=1e-10,outside=1e-9,open=1e-6)


def validate_request(q):
    if (type(q.get('protocol_version')) is not int or q['protocol_version']!=1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1)!=1
            or not isinstance(q.get('operations'),list) or not 1<=len(q['operations'])<=16):
        raise ValueError('grip aliases require bounded protocol 1 recipes')
    seen=set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op)!={'op','id','case'} or op['op']!='grip_alias'
                or not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None
                or op['id'] in seen or op['case'] not in CASES):
            raise ValueError('invalid grip alias recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1,iterations=1,operations=[dict(op='grip_alias',id='grip_alias_'+case,case=case) for case in CASES])


def run(op,host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino,System=host['Rhino'],host['System'];doc=Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand():raise ValueError('grip aliases require idle execution')
    if list(doc.Objects):raise ValueError('grip aliases require an empty owned document')
    if op['case']=='smoothed_circle':
        g=Rhino.Geometry.ArcCurve(Rhino.Geometry.Circle(Rhino.Geometry.Plane.WorldXY,2.))
    else:
        g=Rhino.Geometry.NurbsCurve(3,False,3,4)
        for i,p in enumerate([[0.,0.,0.],[3.,0.,0.],[3.,2.,0.],[DELTAS[op['case']],0.,0.]]):g.Points.SetPoint(i,host['_point'](p))
        for i,t in enumerate([0.,0.,1.,2.,2.]):g.Knots[i]=t
    try:key=doc.Objects.AddCurve(g)
    finally:g.Dispose()
    if key==System.Guid.Empty:raise ValueError('grip alias insertion failed')
    def snapshot():
        o=doc.Objects.FindId(key)
        return dict(curve=host['_nurbs_curve_definition'](o.Geometry.ToNurbsCurve()),closed=bool(o.Geometry.IsClosed),
                    grips=[host['_xyz'](p.CurrentLocation) for p in (o.GetGrips() or [])])
    try:
        obj=doc.Objects.FindId(key);obj.GripsOn=True;before=snapshot();result=dict(before=before)
        if op['case']=='smoothed_circle':
            from join_probe import observe_command
            from smooth_probe import MODES
            for p in obj.GetGrips():
                if int(p.Index) in (0,1):p.Select(True)
            success,after,events=observe_command(Rhino.Commands.Command,'Smooth',lambda:Rhino.RhinoApp.RunScript('_-Smooth '+MODES['object_x'],True),snapshot,lambda:[],True)
            if not success:raise ValueError('grip alias Smooth failed')
            result.update(after=after,events=events)
        obj=doc.Objects.FindId(key);obj.GripsOn=False;obj.GripsOn=True
        result['rebuilt']=snapshot()
        return result,0
    finally:
        obj=doc.Objects.FindId(key)
        if obj is not None:
            obj.GripsOn=False;doc.Objects.Delete(key,True)
