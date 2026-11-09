"""Reproduce scoped native findings without qualifying rejected refinement code."""
import gzip,json,math
from .surface_average_refinement_evidence import ROOT,PROOF,BASELINE,collect,composition,prototypes,sha


def corner_curvature(s,end):
    """Rational Bezier corner jet, independently derived from native controls."""
    p,q=s['degree'];n,m=s['control_count']
    if (n,m)!=(p+1,q+1):raise ValueError('corner example must be a Bezier support')
    controls=s['control_points'];indices=[n-1,n-2,n-3]if end else[0,1,2]
    c=[controls[i]for i in indices];point=c[0]['point'];w=[x['weight']for x in c]
    d=p/(s['domain_u'][1]-s['domain_u'][0]);dd=(p-1)/(s['domain_u'][1]-s['domain_u'][0]);sign=1 if end else-1
    h=[[x['weight']*(x['point'][i]-point[i])for i in range(3)]for x in c]
    wu=sign*d*(w[0]-w[1])
    du=[sign*d*(h[0][i]-h[1][i])/w[0]for i in range(3)]
    duu=[(dd*d*(h[0][i]-2*h[1][i]+h[2][i])-2*du[i]*wu)/w[0]for i in range(3)]
    # Origin-relative h[0] is zero, so its wuu term vanishes.
    adjacent=controls[n+indices[0]];dv=[q/(s['domain_v'][1]-s['domain_v'][0])*adjacent['weight']/w[0]*(adjacent['point'][i]-point[i])for i in range(3)]
    normal=[du[1]*dv[2]-du[2]*dv[1],du[2]*dv[0]-du[0]*dv[2],du[0]*dv[1]-du[1]*dv[0]];length=math.hypot(*normal)
    return sum(a*b for a,b in zip(normal,duu))/length/sum(a*a for a in du)


def build(proof=PROOF):
    rows,paths=collect(proof);models,prototype_paths=prototypes(proof);paths.update(prototype_paths);calls=[];ambiguities=[]
    for key,v in rows.items():
        audit=composition(v)if v['prescribed_case'].get('average_pass_audit')else[];calls.extend(audit)
        values=[(0,v)]+[(s['round'],s['value'])for s in audit if s['status']=='returned']
        for stage,value in values:
            recipe=value['prescribed_case'];queries=value.get('boundary_parameter_audit',[])
            if recipe.get('boundary_parameter_audit'):
                counts=[b['surfaces'][0]['control_count'][1]for b in value['inputs']]
                if len(queries)!=sum(counts):raise ValueError('missing inverse parameter queries')
                expected=[(source,j)for source,count in enumerate(counts)for j in range(count)]
                for query,(source,j)in zip(queries,expected):
                    s=value['inputs'][source]['surfaces'][0];degree=s['degree'][1];vparam=sum(s['knots_v'][j+1:j+degree+1])/degree
                    if query['source']!=source or abs(query['v']-vparam)>1e-12:raise ValueError('inverse query sites')
                    delta=max(abs(query['surface_uv'][0]-query['u']),abs(query['surface_uv'][1]-query['v']))
                    distance=math.dist(query['direct']['point'],query['surface']['point'])
                    if delta>1e-6 and distance<=1e-9:
                        ambiguities.append(dict(id=key,round=stage,source=source,requested_uv=[query['u'],query['v']],
                            surface_uv=query['surface_uv'],curve_v=query['curve_v'],image_distance=distance))
                    if recipe.get('edge_parameter_audit')and not {'edge_uv','edge','edge_point'}.issubset(query):raise ValueError('missing edge-origin query')
    invalid=[s for s in calls if s['status']=='invalid']
    if (len(calls),len(invalid))!=(40,4):raise ValueError('diagnostic pass inventory')
    sweeps=[]
    for flags in ['00','11']:
        values=[rows['average_refine_tolerance_curvature_'+flags+'_percent_'+str(x)]for x in [1e-5,100.]]
        base=rows['average_refine_polynomial_curvature_'+flags]
        if any((v['output'],v['target'])!=(base['output'],base['target'])for v in values):raise ValueError('curvature sweep changed')
        sweeps.append(dict(flags=flags,readback_percentages=[1e-5,.05,100.],identical_output_definitions=True))
    example=rows['average_refine_polynomial_curvature_10'];k=[corner_curvature(example[key]['surfaces'][0],key=='output')for key in ['output','target']]
    return dict(scope='Native averaged refinement and inverse-parameter investigation; neither prototype is a production implementation.',
        baseline_revision=BASELINE,successful_headless_launches=6,retained_failed_launches=1,native_primary_observations=len(rows),
        refinement_enabled_observations=sum(v['configured_settings']['enabled']for v in rows.values()),
        diagnostic_composed_calls=len(calls),returned_valid_diagnostic_calls=len(calls)-len(invalid),invalid_diagnostic_calls=len(invalid),
        frozen_prototypes=models,averaged_refinement_qualified=False,continuous_continuity_qualified=False,performance_qualified=False,
        inverse_ambiguity_examples=ambiguities,curvature_percentage_sweeps=sweeps,
        curvature_contact_example=dict(id='average_refine_polynomial_curvature_10',
            counts=[example[key]['surfaces'][0]['control_count']for key in ['output','target']],
            derived_normal_curvatures=k,relative_difference=abs(k[0]-k[1])/max(abs(x)for x in k),
            requested_percent=example['configured_settings']['curvature']),
        findings=['Repeated simultaneous averaging with raw traversal flags reproduces many native results; both experimental replays remain rejected at the 1e-9 geometry gate.',
            'Native surface and curve inverse queries can select different parameters for the same point on a folded intermediate boundary.',
            'A blanket inverse lookup reduces the matched pilot inventory from 25/30 to 24/30. It is not an accepted replacement fitter.',
            'The recorded curvature percentage sweep leaves output definitions unchanged for these two scoped cases. The stopping policy remains unresolved.'],
        limits=['All refinement cases remain outside production qualification. The existing API still rejects averaged refinement.',
            'Composed public calls use fresh reconstructions of prior native outputs. They diagnose behavior and do not prove equivalence to internal refinement.',
            'Four invalid composed calls and the original failed launch are retained explicitly; no failure is counted as a successful geometry replay.'],
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(paths)})


if __name__=='__main__':
    value=build();(ROOT/'docs/surface-average-refinement-investigation.json').write_text(json.dumps(value,indent=2,allow_nan=False)+'\n')
    print('Recorded averaged refinement investigation; production remains unqualified')
