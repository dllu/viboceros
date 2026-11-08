"""Public contact-point controls using authored sources and pre-command pick witnesses."""


def request():
    import gzip,json
    from pathlib import Path
    root=Path(__file__).resolve().parent;operations=[]
    wanted={'continuation_nurbs_smooth_joined','continuation_disjoint_smooth','continuation_smooth_bridge_0_6','continuation_smooth_bridge_0_7','continuation_smooth_bridge_0_8','continuation_smooth_bridge_1_0','continuation_smooth_span_0_25','continuation_smooth_span_2_0'}
    for family in ['pilot','holdouts','verification']:
        rows=json.loads(gzip.decompress((root/('observations/fillet_continuation_'+family+'.json.gz')).read_bytes()))['results']
        for row in rows:
            if row['id']not in wanted:continue
            value=row['value'];case=value['prescribed_case']
            operations.append(dict(op='fillet_continuation_sdk',id=row['id'],curves=case['curves'],radius=case['actions'][1],picks=[c['sight_point']for c in value['input_trace'][0]['clicks']]))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=operations)


def validate(op):
    if not isinstance(op,dict) or set(op)!={'op','id','curves','radius','picks'} or op.get('op')!='fillet_continuation_sdk' or len(op.get('curves',[]))!=2 or len(op.get('picks',[]))!=2 or op.get('radius',0.)<=0.:
        raise ValueError('invalid prescribed Fillet contact diagnostic')


def run(op,host,iterations,tolerance):
    validate(op);R=host['Rhino'];G=R.Geometry
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects):raise ValueError('empty owned document required')
    curves=[host['_object_source'](c,tolerance)for c in op['curves']];extended=None
    try:
        before=[host['_nurbs_curve_definition'](c)for c in curves]
        seeds=[c.ClosestPoint(G.Point3d(*p))[1]for c,p in zip(curves,op['picks'])]
        domain=curves[1].Domain;width=domain.Length
        extended=curves[1].Extend(domain.T0-3.*width,domain.T1+width)
        def observe(source):
            ok,t0,t1,plane=G.Curve.GetFilletPoints(curves[0],source,op['radius'],seeds[0],seeds[1])
            result=dict(success=bool(ok))
            if ok:
                arc=G.Curve.CreateFillet(curves[0],source,op['radius'],seeds[0],seeds[1])
                result.update(parameters=[float(t0),float(t1)],center=host['_xyz'](plane.Origin),contacts=[host['_xyz'](c.PointAt(t))for c,t in zip([curves[0],source],[t0,t1])],arc_valid=bool(arc.IsValid),source_curvature=float(source.CurvatureAt(t1).Length),distance=sum(c.GetLength(G.Interval(min(a,b),max(a,b)))for c,a,b in zip([curves[0],source],seeds,[t0,t1])))
                if arc.IsValid:result.update(arc_angle=float(arc.Angle),arc_end_tangent=host['_xyz'](arc.TangentAt(arc.AngleDomain.T1)),source_tangent=host['_xyz'](source.TangentAt(t1)))
            return result
        return dict(original=observe(curves[1]),extended=observe(extended)if extended is not None else None,source_unchanged=before==[host['_nurbs_curve_definition'](c)for c in curves]),0
    finally:
        if extended is not None:extended.Dispose()
        for curve in curves:curve.Dispose()
