"""Public composite/segment clocks beside actual control-removal commands."""
try:
    from . import remove_control_point_probe as base
except (ImportError, ValueError):
    import remove_control_point_probe as base


def plain(op):
    return dict(op, op='remove_control_point_command')


def validate(op):
    if not isinstance(op,dict) or op.get('op')!='remove_control_point_composite_parameters':
        raise ValueError('invalid composite parameter recipe')
    base.validate(plain(op))
    if op['family']!='polycurve' or len(op['actions'])!=1 or not isinstance(op['actions'][0],dict):
        raise ValueError('requires one composite point')


def run(op,host,iterations,tolerance):
    validate(op)
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    rhino=base.base.Forward(R);app=base.base.Forward(R.RhinoApp);rhino.RhinoApp=app
    state={}
    def execute(macro,echo):
        if not macro.startswith('_RemoveControlPoint '):return R.RhinoApp.RunScript(macro,echo)
        source=list(doc.Objects)[0].Geometry
        nurbs=source.ToNurbsCurve()
        try:
            action=op['actions'][0]
            point=G.Point3d(*action['point'])if 'point'in action else nurbs.Points[action['grip']].Location
            ok,t=source.ClosestPoint(point);nk,nt=nurbs.ClosestPoint(point)
            index=source.SegmentIndex(t);local=source.SegmentCurveParameter(t)
            state.update(found=bool(ok),parameter=t,nurbs_parameter=nt,nurbs_found=bool(nk),
                segment=index,local_parameter=local,point=host['_xyz'](point),segments=[])
            for i in range(source.SegmentCount):
                leaf=source.SegmentCurve(i);n=leaf.ToNurbsCurve()
                try:
                    ok,ct=n.ClosestPoint(point)
                    raw=n.PointAt(t)
                    state['segments'].append(dict(index=i,domain=[n.Domain.T0,n.Domain.T1],
                        closest=ct,found=bool(ok),raw_valid=bool(raw.IsValid),raw_point=host['_xyz'](raw)if raw.IsValid else None,
                        controls=[host['_xyz'](n.Points[j].Location)for j in range(n.Points.Count)]))
                finally:n.Dispose()
        finally:nurbs.Dispose()
        return R.RhinoApp.RunScript(macro,echo)
    app.RunScript=execute;local=dict(host);local['Rhino']=rhino
    value,elapsed=base.run(plain(op),local,iterations,tolerance)
    value.update(recipe=op,parameter_queries=state)
    return value,elapsed
