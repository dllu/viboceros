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
    return run_recipe(CASES[op['case']],host,iterations,tolerance)


def run_recipe(recipe,host,iterations,tolerance):
    """Execute one authored recipe without sharing or mutating its inputs."""
    if iterations!=1:raise ValueError('one surface matching observation required')
    import brep_join_probe
    G=host['Rhino'].Geometry;recipe=copy.deepcopy(recipe)
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
            if recipe.get('insert_transverse_midpoints'):
                values=sorted(set(float(x)for x in s.KnotsV))
                for a,b in zip(values,values[1:]):
                    if not s.KnotsV.InsertKnot(a+(b-a)*.5):raise ValueError('diagnostic transverse insertion failed')
            b=s.DuplicateBrep() if isinstance(s,G.Brep) else s.ToBrep();breps.append(b)
            if b is None or not b.IsValid:raise ValueError('invalid authored matching source')
        before=[record(b) for b in breps]
        parameter_queries=[]
        if recipe.get('boundary_parameter_audit'):
            for index,s in enumerate(sources):
                side=recipe['side'if index==0 else'target_side']
                if side not in ['East','West']:raise ValueError('boundary parameter audit requires East/West')
                u=s.Domain(0).Max if side=='East'else s.Domain(0).Min
                degree=s.Degree(1);knots=[float(x)for x in s.KnotsV]
                curve=s.IsoCurve(1,u)
                try:
                    for j in range(s.Points.CountV):
                        v=sum(knots[j:j+degree])/degree;p=s.PointAt(u,v)
                        ok,cv=curve.ClosestPoint(p)
                        if not ok:raise ValueError('diagnostic boundary inverse failed')
                        ok,su,sv=s.ClosestPoint(p)
                        if not ok:raise ValueError('diagnostic surface inverse failed')
                        def evaluated(a,b):
                            ok,point,derivatives=s.Evaluate(a,b,1)
                            if not ok:raise ValueError('diagnostic surface evaluation failed')
                            return dict(point=[float(point.X),float(point.Y),float(point.Z)],
                                du=[float(derivatives[0].X),float(derivatives[0].Y),float(derivatives[0].Z)],
                                dv=[float(derivatives[1].X),float(derivatives[1].Y),float(derivatives[1].Z)])
                        query=dict(source=index,u=float(u),v=float(v),
                            curve_v=float(cv),surface_uv=[float(su),float(sv)],
                            direct=evaluated(u,v),curve=evaluated(u,cv),surface=evaluated(su,sv))
                        if recipe.get('edge_parameter_audit'):
                            edge_point=curve.PointAt(v);ok,eu,ev=s.ClosestPoint(edge_point)
                            if not ok:raise ValueError('diagnostic edge surface inverse failed')
                            query.update(edge_uv=[float(eu),float(ev)],edge=evaluated(eu,ev),
                                edge_point=[float(edge_point.X),float(edge_point.Y),float(edge_point.Z)])
                        parameter_queries.append(query)
                finally:curve.Dispose()
        continuity={'None':getattr(G.Continuity,'None'),'Position':G.Continuity.C0_continuous,
            'Tangency':G.Continuity.G1_continuous,'Curvature':G.Continuity.G2_continuous}
        settings=G.MatchSrfSettings(continuity[recipe['continuity']],continuity[recipe['other_end']])
        settings.PreserveIso=getattr(G.PreserveIsoCurveMethod,recipe['iso'])
        settings.Average=recipe['average'];settings.MatchClosestPoints=recipe['closest_points']
        settings.ReverseMatchDirection=recipe['reverse_match'];settings.ReverseAverageTargetDirection=recipe['reverse_target']
        settings.EnableRefinement(recipe['refine'],recipe.get('distance_tolerance',tolerance['absolute']),recipe.get('angle_tolerance',tolerance['angular']),recipe.get('curvature_tolerance',.05))
        success,matched,changed_target=G.Brep.CreateFromMatch(edge(breps[0],recipe['side']),edge(breps[1],recipe['target_side']),settings)
        if [record(b) for b in breps]!=before:raise ValueError('matching API mutated an owned input')
        if matched is not None and not matched.IsValid:raise ValueError('invalid surface match result')
        if changed_target is not None and not changed_target.IsValid:raise ValueError('invalid averaged target')
        value=dict(prescribed_case=recipe,inputs=before,source_unchanged=True,success=bool(success),
            output=record(matched) if matched is not None else None,
            target=record(changed_target) if changed_target is not None else None)
        if recipe.get('settings_audit'):
            value['configured_settings']=dict(enabled=bool(settings.RefineMatch),
                distance=float(settings.RefinePositionalTolerance),angle=float(settings.RefineAngleToleranceRadians),
                curvature=float(settings.RefineCurvatureTolerance))
        if recipe.get('boundary_parameter_audit'):value['boundary_parameter_audit']=parameter_queries
        if recipe.get('match_to_curve_levels'):
            value['prefit_audit']=[]
            variants=[('edge',False,0.)]
            if recipe.get('match_to_curve_variants'):
                variants=[(kind,reverse,10.)for kind in ['edge','iso']for reverse in [False,True]]+[
                    ('iso',False,0.),('iso',False,-1.)]
            for kind,reverse,limit in variants:
                curve=(edge(breps[1],recipe['target_side']).DuplicateCurve() if kind=='edge' else
                    sources[1].IsoCurve(1,sources[1].Domain(0).Min))
                try:
                    if reverse:curve.Reverse()
                    for level in recipe['match_to_curve_levels']:
                        prefit=sources[0].MatchToCurve(getattr(G.IsoStatus,recipe['side']),
                            curve,limit,limit,recipe['distance_tolerance'],level)
                        prefit_brep=None
                        try:
                            if prefit is not None:prefit_brep=prefit.ToBrep()
                            value['prefit_audit'].append(dict(level=level,kind=kind,reverse=reverse,limit=limit,
                                output=record(prefit_brep) if prefit_brep is not None else None))
                        finally:
                            if prefit_brep is not None:prefit_brep.Dispose()
                            if prefit is not None:prefit.Dispose()
                finally:curve.Dispose()
            if [record(b)for b in breps]!=before:raise ValueError('prefit audit mutated an owned input')
            for original,source in zip(before,sources):
                check=source.ToBrep()
                try:
                    if record(check)!=original:raise ValueError('prefit audit mutated an owned surface')
                finally:check.Dispose()
        if recipe.get('audit') and matched is not None:
            support=matched.Faces[0].UnderlyingSurface();reference=breps[1].Faces[0].UnderlyingSurface()
            edge_curve=reference.IsoCurve(1,reference.Domain(0).Min)
            samples=[]
            try:
                knots=support.ToNurbsSurface()
                try: values=sorted(set(float(x) for x in knots.KnotsV))
                finally: knots.Dispose()
                local_seed=edge_curve.Domain.Min
                curve_deviations=[]
                if recipe.get('curve_deviation_audit'):
                    source_curve=support.IsoCurve(1,support.Domain(0).Max)
                    try:
                        for accuracy in [1e-8,1e-6,1e-5]:
                            result=G.Curve.GetDistancesBetweenCurves(source_curve,edge(breps[1],recipe['target_side']),accuracy)
                            curve_deviations.append(dict(tolerance=accuracy,success=bool(result[0]),values=[float(x) for x in result[1:]] if result[0] else None))
                    finally:source_curve.Dispose()
                for a,b in zip(values,values[1:]):
                    for i in range(9):
                        v=a+(b-a)*i/8.;point=support.PointAt(support.Domain(0).Max,v)
                        ok,t=edge_curve.ClosestPoint(point)
                        if not ok:raise ValueError('refinement audit closest point failed')
                        p=reference.PointAt(reference.Domain(0).Min,t)
                        n=support.NormalAt(support.Domain(0).Max,v);m=reference.NormalAt(reference.Domain(0).Min,t)
                        if n*m<0:m=-m
                        surface_query={}
                        if recipe.get('inverse_surface_audit'):
                            ok,iu,iv=support.ClosestPoint(p)
                            if not ok:raise ValueError('inverse surface closest audit failed')
                            surface_query['inverse_surface_closest']=dict(u=float(iu),v=float(iv),distance=float(p.DistanceTo(support.PointAt(iu,iv))))
                        if recipe.get('local_closest_audit'):
                            ok,local=edge_curve.LocalClosestPoint(point,local_seed)
                            if not ok:raise ValueError('local closest audit failed')
                            surface_query['local_closest']=dict(seed=float(local_seed),parameter=float(local),distance=float(point.DistanceTo(edge_curve.PointAt(local))))
                            local_seed=local
                        if recipe.get('surface_closest_audit'):
                            ok,su,sv=reference.ClosestPoint(point)
                            if not ok:raise ValueError('surface closest audit failed')
                            surface_query.update(surface_closest=dict(u=float(su),v=float(sv),distance=float(point.DistanceTo(reference.PointAt(su,sv)))))
                        samples.append(dict(surface_query,v=v,target_v=t,distance=point.DistanceTo(p),angle=G.Vector3d.VectorAngle(n,m)))
            finally:edge_curve.Dispose()
            doc=host['Rhino'].RhinoDoc.ActiveDoc
            value['audit']=dict(settings=dict(enabled=bool(settings.RefineMatch),
                distance=float(settings.RefinePositionalTolerance),angle=float(settings.RefineAngleToleranceRadians),
                curvature=float(settings.RefineCurvatureTolerance)),
                document=dict(absolute=float(doc.ModelAbsoluteTolerance),angular=float(doc.ModelAngleToleranceRadians)),
                samples=samples)
            if recipe.get('curve_deviation_audit'):value['audit']['curve_deviations']=curve_deviations
        return value,0
    finally:
        for obj in [matched,changed_target]+breps+sources:
            if obj is not None:obj.Dispose()
