"""Owned public Brep.CreateFromMatch observations; no proprietary inspection."""
import copy
if __package__:
    from .match_surface_cases import CASES
else:
    from match_surface_cases import CASES


def validate(op):
    if (not isinstance(op,dict) or set(op)!={'op','id','case'} or op.get('op')!='match_surface_sdk'
            or op.get('id')!=op.get('case') or op.get('case') not in CASES):
        raise ValueError('invalid prescribed surface-match SDK case')


def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('one surface matching observation required')
    import brep_join_probe
    G=host['Rhino'].Geometry;recipe=copy.deepcopy(CASES[op['case']])
    sources=[];breps=[];matched=None;changed_target=None
    def record(b):return brep_join_probe.geometry_record(b,tolerance,host)
    def edge(b,side):
        iso=getattr(G.IsoStatus,side)
        trims=[t for t in b.Faces[0].Loops[0].Trims if t.IsoStatus==iso]
        if len(trims)!=1 or trims[0].Edge is None:raise ValueError('unique natural boundary required')
        return trims[0].Edge
    try:
        for definition in recipe['sources']:
            s=host['_object_source'](definition,tolerance);sources.append(s)
            b=s.DuplicateBrep() if isinstance(s,G.Brep) else s.ToBrep();breps.append(b)
            if b is None or not b.IsValid:raise ValueError('invalid authored matching source')
        before=[record(b) for b in breps]
        continuity={'None':getattr(G.Continuity,'None'),'Position':G.Continuity.C0_continuous,
            'Tangency':G.Continuity.G1_continuous,'Curvature':G.Continuity.G2_continuous}
        settings=G.MatchSrfSettings(continuity[recipe['continuity']],continuity[recipe['other_end']])
        settings.PreserveIso=getattr(G.PreserveIsoCurveMethod,recipe['iso'])
        settings.Average=recipe['average'];settings.MatchClosestPoints=recipe['closest_points']
        settings.ReverseMatchDirection=recipe['reverse_match'];settings.ReverseAverageTargetDirection=recipe['reverse_target']
        settings.EnableRefinement(recipe['refine'],tolerance['absolute'],recipe.get('angle_tolerance',tolerance['angular']),.05)
        success,matched,changed_target=G.Brep.CreateFromMatch(edge(breps[0],recipe['side']),edge(breps[1],recipe['target_side']),settings)
        if [record(b) for b in breps]!=before:raise ValueError('matching API mutated an owned input')
        if matched is not None and not matched.IsValid:raise ValueError('invalid surface match result')
        if changed_target is not None and not changed_target.IsValid:raise ValueError('invalid averaged target')
        value=dict(prescribed_case=recipe,inputs=before,source_unchanged=True,success=bool(success),
            output=record(matched) if matched is not None else None,
            target=record(changed_target) if changed_target is not None else None)
        if recipe.get('audit') and matched is not None:
            support=matched.Faces[0].UnderlyingSurface();reference=breps[1].Faces[0].UnderlyingSurface()
            edge_curve=reference.IsoCurve(1,reference.Domain(0).Min)
            samples=[]
            try:
                knots=support.ToNurbsSurface()
                try: values=sorted(set(float(x) for x in knots.KnotsV))
                finally: knots.Dispose()
                for a,b in zip(values,values[1:]):
                    for i in range(9):
                        v=a+(b-a)*i/8.;point=support.PointAt(support.Domain(0).Max,v)
                        ok,t=edge_curve.ClosestPoint(point)
                        if not ok:raise ValueError('refinement audit closest point failed')
                        p=reference.PointAt(reference.Domain(0).Min,t)
                        n=support.NormalAt(support.Domain(0).Max,v);m=reference.NormalAt(reference.Domain(0).Min,t)
                        if n*m<0:m=-m
                        samples.append(dict(v=v,target_v=t,distance=point.DistanceTo(p),angle=G.Vector3d.VectorAngle(n,m)))
            finally:edge_curve.Dispose()
            doc=host['Rhino'].RhinoDoc.ActiveDoc
            value['audit']=dict(settings=dict(enabled=bool(settings.RefineMatch),
                distance=float(settings.RefinePositionalTolerance),angle=float(settings.RefineAngleToleranceRadians),
                curvature=float(settings.RefineCurvatureTolerance)),
                document=dict(absolute=float(doc.ModelAbsoluteTolerance),angular=float(doc.ModelAngleToleranceRadians)),
                samples=samples)
        return value,0
    finally:
        for obj in [matched,changed_target]+breps+sources:
            if obj is not None:obj.Dispose()
