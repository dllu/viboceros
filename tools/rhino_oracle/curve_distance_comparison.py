"""Compare interval-query scalars and retain native overlap/metric disagreements."""
from fractions import Fraction as F
import argparse,json,math
from pathlib import Path
from .client import compare_responses


def bezier_point(curve,parameter):
    """Independent exact evaluation of the captured polynomial Bezier inputs."""
    n=curve['degree'];cp=curve['control_points'];t=F(parameter)
    if (curve['type']!='nurbs' or len(cp)!=n+1 or curve['knots']!=[0]*(n+1)+[1]*(n+1)
        or any(F(p['weight'])!=1 for p in cp) or not 0<=t<=1):raise ValueError('polynomial unit-domain Bezier required')
    return [sum(F(cp[i]['point'][axis])*math.comb(n,i)*t**i*(1-t)**(n-i)for i in range(n+1))for axis in range(3)]


def comparison(request,local,native):
    eps=F(1e-6);rows=[];counts=dict(status_matches=0,maximum_scalar_matches=0,minimum_scalar_matches=0,scalar_pair_matches=0,source_pair_counterexamples=0)
    registrations={row['value']['command_registered']for row in local['results']}
    if len(registrations)!=1:raise ValueError('inconsistent local command registration')
    if not len(request['operations'])==len(local['results'])==len(native['results']):raise ValueError('recipe/result count mismatch')
    for op,a,b in zip(request['operations'],local['results'],native['results']):
        if op['id']!=a['id'] or a['id']!=b['id']:raise ValueError('recipe/result identity mismatch')
        av,bv=a['value'],b['value'];x,y=av['sdk'],bv['sdk'];row=dict(id=op['id'],backend_created=x['created'],native_created=y['created'],scalars_within_1e_minus_6={})
        if x['created']==y['created']:counts['status_matches']+=1
        if x['created']and y['created']:
            for key in ('maximum','minimum'):
                error=abs(F(x[key]['distance'])-F(y[key]['distance']));passed=error<=eps
                row['scalars_within_1e_minus_6'][key]=passed;row['exact_'+key+'_scalar_difference']=str(error)
                if passed:counts[key+'_scalar_matches']+=1
            if all(row['scalars_within_1e_minus_6'].values()):counts['scalar_pair_matches']+=1
            if F(x['minimum']['distance'])<F(y['minimum']['distance'])-eps:
                p=[bezier_point(c,t)for c,t in zip(op['curves'],x['minimum']['parameters'])]
                residuals=[sum((u-F(v))**2 for u,v in zip(point,stored))for point,stored in zip(p,x['minimum']['points'])]
                if not all(r<=eps**2 for r in residuals):raise ValueError('reported witness is not on the original polynomial image')
                squared=sum((u-v)**2 for u,v in zip(*p));margin=F(1e-4)
                if not squared<(F(y['minimum']['distance'])-margin)**2:raise ValueError('source pair does not support the minimum diagnostic')
                row['whole_interval_minimum_counterexample']=dict(exact_squared_source_pair_distance=str(squared),exact_squared_witness_residuals=list(map(str,residuals)),reference_minimum_exceeds_this_pair_by_at_least=1e-4,scope='Whole source intervals; no assertion about a native correspondence rule.')
                counts['source_pair_counterexamples']+=1
        row['matches_recorded_scalars_and_status']=x['created']==y['created']and (not x['created']or all(row['scalars_within_1e_minus_6'].values()))
        rows.append(row)
    return dict(schema_version=1,absolute_epsilon=1e-6,reference_derived_inputs=False,command_registered=registrations.pop(),
        complete_native_command_parity_qualified=False,continuous_global_extrema_qualified=False,performance_qualified=False,
        method='Exact binary-rational scalar differences at 1e-6, preserving all statuses and raw witnesses. Two closer source pairs are evaluated independently with exact polynomial Bernstein arithmetic. No tied-witness position/direction equivalence or native overlap correspondence is inferred.',
        cases=len(rows),counts=counts,recorded_scalar_and_status_comparison_passed=all(row['matches_recorded_scalars_and_status']for row in rows),results=rows,
        raw_protocol_comparison=compare_responses(local,native,absolute_epsilon=1e-6,relative_epsilon=0).as_dict())


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('request',type=Path);p.add_argument('local',type=Path);p.add_argument('native',type=Path);p.add_argument('output',type=Path)
    a=p.parse_args();r=comparison(*[json.loads(path.read_text())for path in (a.request,a.local,a.native)])
    a.output.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(dict(counts=r['counts'],comparison_passed=r['recorded_scalar_and_status_comparison_passed'])))
if __name__=='__main__':main()
