"""Compare source curvature samples; native tessellation stays distinct."""
import argparse,json,math
from pathlib import Path

def comparison(request,local,native):
    if not len(request['operations'])==len(local['results'])==len(native['results']):raise ValueError('graph case count mismatch')
    rows=[]
    for op,a,b in zip(request['operations'],local['results'],native['results']):
        if not op['id']==a['id']==b['id']:raise ValueError('graph case identity mismatch')
        x,y=a['value'],b['value']
        if not x['settings']==y['settings']==dict(scale=op['scale'],density=op['density']):raise ValueError('graph setting mismatch')
        if len(x['samples'])!=17 or len(y['samples'])!=17:raise ValueError('graph sample count mismatch')
        errors={key:0. for key in ('point','curvature')}
        for p,q in zip(x['samples'],y['samples']):
            if p['fraction']!=q['fraction']:raise ValueError('graph sample parameter mismatch')
            for key in errors:
                if len(p[key])!=3 or len(q[key])!=3 or not all(math.isfinite(v)for v in p[key]+q[key]):raise ValueError('invalid graph vector')
                errors[key]=max(errors[key],math.dist(p[key],q[key]))
        row=dict(id=op['id'],maximum_sample_errors=errors,sampled_points_and_curvatures_within_1e_minus_6=all(v<=1e-6 for v in errors.values()),sources_unchanged=x['inputs_unchanged']and y['inputs_unchanged'],native_extracted_graph_objects=len(y['outputs']),local_hair_samples=x['hair_count'])
        if op['source']['type']=='circle':
            center=op['source']['center'];radius=op['source']['radius']
            points=[p for o in y['outputs'] for p in o.get('points',[])]
            if not points:raise ValueError('no native circle graph extraction')
            factor=2.**((op['scale']-100.)/2.)
            expected=radius+factor/radius
            row['native_circle_envelope_relative_error']=max(abs(math.dist(center,p)-expected)/expected for p in points)
            row['native_circle_scale_rule_matches']=row['native_circle_envelope_relative_error']<=1e-12
            row['local_maximum_hair_length_error']=abs(x['maximum_hair_length']-factor/radius)
        rows.append(row)
    return dict(schema_version=1,cases=len(rows),sample_pairs=17*len(rows),matched_sample_cases=sum(r['sampled_points_and_curvatures_within_1e_minus_6']for r in rows),unchanged_source_cases=sum(r['sources_unchanged']for r in rows),complete_native_graph_parity_qualified=False,continuous_global_curvature_qualified=False,performance_qualified=False,method='17 common source-chart samples per case, using the NURBS chart for ellipse admission. Original recipe geometry is the only input. Native graph extraction retains actual polylines; independent local knot-span sampling is not asserted to match native tessellation or hair density. Circle envelope ratios independently check the exponential display multiplier.',results=rows)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ('request','local','native','output'):parser.add_argument(name,type=Path)
    args=parser.parse_args();result=comparison(*[json.loads(p.read_text())for p in (args.request,args.local,args.native)])
    args.output.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:result[k]for k in ('cases','sample_pairs','matched_sample_cases','unchanged_source_cases')}))
if __name__=='__main__':main()
