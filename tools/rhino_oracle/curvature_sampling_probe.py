"""Recover sparse graph stations through public span closest-point queries."""
if globals().get('__package__'):
    from .curvature_graph_probe import validate as graph_validate,run as graph_run
else:
    from curvature_graph_probe import validate as graph_validate,run as graph_run
def recipe(op):
    q=dict(op);q['op']='curvature_graph';q.setdefault('sample_density',2);return q
def validate(op):
    if not isinstance(op,dict)or op.get('op')!='curvature_sampling':raise ValueError('invalid curvature sampling recipe')
    graph_validate(recipe(op))
    if 'sample_density'in op and(type(op['sample_density'])is not int or not 1<=op['sample_density']<=10):raise ValueError('invalid sampling density')
def run(op,host,iterations,tolerance):
    validate(op);R=host['Rhino'];G=R.Geometry
    value,elapsed=graph_run(recipe(op),host,iterations,tolerance)
    source=host['_object_source'](op['source'],tolerance);curve=source.ToNurbsCurve();pieces=[]
    try:
        for i in range(curve.SpanCount):pieces.append(curve.Trim(curve.SpanDomain(i)))
        factor=2.**((op['scale']-100.)/2.);out=[]
        for output in value['outputs']:
            points=output.get('points')
            if points is None:raise ValueError('polyline graph required')
            count=len(points)
            indices=sorted(set(list(range(min(9,count)))+[count-1-i for i in range(min(5,count))]+[int((count-1)*i/32)for i in range(33)]))
            stations=[]
            for index in indices:
                q=G.Point3d(*points[index]);candidates=[]
                for span,piece in enumerate(pieces):
                    ok,t=piece.ClosestPoint(q)
                    if not ok:continue
                    for side in (G.CurveEvaluationSide.Below,G.CurveEvaluationSide.Above):
                        jet=piece.DerivativeAt(t,2,side)
                        if jet is None or len(jet)!=3:continue
                        p,d1,d2=jet;speed2=d1.SquareLength
                        if speed2<=0:continue
                        along=(d1.X*d2.X+d1.Y*d2.Y+d1.Z*d2.Z)/speed2
                        k=G.Vector3d((d2.X-along*d1.X)/speed2,(d2.Y-along*d1.Y)/speed2,(d2.Z-along*d1.Z)/speed2)
                        tip=G.Point3d(p.X-factor*k.X,p.Y-factor*k.Y,p.Z-factor*k.Z)
                        candidates.append((tip.DistanceTo(q),float(t),span))
                if not candidates:raise ValueError('no source station candidate')
                residual,t,span=min(candidates)
                stations.append(dict(index=index,parameter=t,fraction=float(curve.Domain.NormalizedParameterAt(t)),span=span,residual=residual))
            out.append(dict(vertex_count=count,stations=stations))
        tangents=[]
        for i in range(curve.SpanCount):
            span=curve.SpanDomain(i);a=curve.DerivativeAt(span.T0,1,G.CurveEvaluationSide.Above)[1];b=curve.DerivativeAt(span.T1,1,G.CurveEvaluationSide.Below)[1]
            tangents.append(dict(domain=[float(span.T0),float(span.T1)],first=host['_xyz'](a),last=host['_xyz'](b),dot=a.X*b.X+a.Y*b.Y+a.Z*b.Z))
        return dict(settings=value['settings'],sampling_after_setup=value.get('sampling_after_setup'),inputs_unchanged=value['inputs_unchanged'],source_nurbs=host['_nurbs_curve_definition'](curve),graphs=out,span_tangents=tangents,outputs=value['outputs']),elapsed
    finally:
        for piece in pieces:
            if piece is not None:piece.Dispose()
        curve.Dispose();source.Dispose()
