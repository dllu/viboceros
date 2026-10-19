"""Bounded public command sequences that distinguish cyclic deletion order."""
import re

def validate(op):
    if (not isinstance(op,dict) or set(op)-{'op','id','kind','selections','degree','count'} or op.get('op')!='grip_delete_sequence'
            or not isinstance(op.get('id'),str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None
            or op.get('kind') not in ('circle','closed_weighted','closed_uniform') or not isinstance(op.get('selections'),list)
            or not 1<=len(op['selections'])<=4):raise ValueError('invalid deletion sequence')
    for pick in op['selections']:
        if not isinstance(pick,list) or not 1<=len(pick)<=4 or any(type(i)is not int or not 0<=i<16 for i in pick) or len(set(pick))!=len(pick):raise ValueError('invalid sequence grip indices')
    if op['kind']=='closed_uniform':
        if type(op.get('degree'))is not int or not 2<=op['degree']<=6 or type(op.get('count'))is not int or not op['degree']+1<=op['count']<=16:raise ValueError('invalid cyclic sequence basis')
    elif 'degree'in op or 'count'in op:raise ValueError('unexpected sequence basis')

def run(op,host):
    validate(op)
    from delete_grips_probe import create_source
    from join_probe import observe_command
    R=host['Rhino'];doc=R.RhinoDoc.ActiveDoc
    if list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('sequence requires an owned idle document')
    key=None
    def snapshot():
        obj=doc.Objects.FindId(key)
        if obj is None:return None
        return dict(curve=host['_nurbs_curve_definition'](obj.Geometry),grips=[dict(index=int(g.Index),point=host['_xyz'](g.CurrentLocation),selected=bool(g.IsSelected(False)))for g in obj.GetGrips()])
    try:
        if op['kind']=='closed_uniform':
            import math
            degree,count=op['degree'],op['count'];source=R.Geometry.NurbsCurve(3,True,degree+1,count+1)
            for i in range(count+1):
                j=i%count;source.Points.SetPoint(i,host['_point']([3.*math.cos(2.*math.pi*j/count),3.*math.sin(2.*math.pi*j/count),0.]),1.)
            source.Knots.CreateUniformKnots(1.)
        else:source=create_source(op['kind'],host)
        try:key=doc.Objects.AddCurve(source)
        finally:source.Dispose()
        doc.Objects.FindId(key).GripsOn=True
        before=snapshot();steps=[]
        for selected in op['selections']:
            grips=list(doc.Objects.FindId(key).GetGrips())
            if any(i>=len(grips)for i in selected):raise ValueError('sequence index outside live grips')
            for g in grips:g.Select(int(g.Index)in selected)
            _,after,events=observe_command(R.Commands.Command,'Delete',lambda:R.RhinoApp.RunScript('_Delete',False),snapshot,lambda:[],True)
            result=[e['result']for e in events if e['name']=='Delete']
            if len(result)!=1:raise ValueError('sequence command completion missing')
            steps.append(dict(selected=selected,after=after,after_script=snapshot(),result=result[0]))
        return dict(before=before,steps=steps),0
    finally:
        for obj in list(doc.Objects):
            if obj.GripsOn:obj.GripsOn=False
            doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True);doc.ClearRedoRecords()
