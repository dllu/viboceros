"""Summarize station qualification without changing the model parity gate."""
import argparse,hashlib,json,math,re
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
SOURCES=['crates/viboceros-geometry/src/surface_geodesic.rs','crates/viboceros-geometry/src/surface_geodesic/tests.rs','crates/viboceros-geometry/src/surface_curve_offset.rs','crates/viboceros-geometry/src/surface_curve_offset/tests.rs','crates/viboceros-geometry/src/surface_curve_offset/clip.rs','crates/viboceros-geometry/src/surface_curve_offset/clip/tests.rs','crates/viboceros-geometry/src/surface_curve_offset/clip/closed_gap.rs','crates/viboceros-geometry/src/nurbs_surface/offset_extension.rs','crates/viboceros-geometry/src/morph/curve_fit.rs','crates/viboceros-geometry/src/integration/cash_karp.rs','crates/viboceros-geometry/tests/surface_geodesic_native.rs','tools/rhino_oracle/fixtures/surface_curve_offset.json','tools/rhino_oracle/observations/surface_curve_offset.json','tools/rhino_oracle/fixtures/surface_curve_offset_verify.json','tools/rhino_oracle/observations/surface_curve_offset_verify.json','tools/rhino_oracle/fixtures/surface_curve_offset_clipping.json','tools/rhino_oracle/observations/surface_curve_offset_clipping.json','tools/rhino_oracle/report_surface_geodesic.py']
def summarize(log):
    rows=[json.loads(s)for s in re.findall(r'SURFACE_GEODESIC (\{[^\n]+\})',log)]
    q=json.loads((HERE/'fixtures/surface_curve_offset.json').read_text())
    if len(rows)!=24 or [x['id']for x in rows]!=[x['id']for x in q['operations']]:raise ValueError('one complete ordered station replay is required')
    for row in rows:
        error=row['maximum_error']
        if not math.isfinite(error)or error<0 or row['errors']or row['qualified']!=(error<1e-6):raise ValueError('invalid station qualification row')
    fits=[json.loads(s)for s in re.findall(r'SURFACE_CURVE_OFFSET (\{[^\n]+\})',log)]
    if len(fits)!=24 or [x['id']for x in fits]!=[x['id']for x in q['operations']]:raise ValueError('one complete ordered fitted-curve replay is required')
    for fit in fits:
        error=fit['maximum_bidirectional_sample_error']
        if fit['status']not in('curve','empty')or not math.isfinite(error)or error<0 or fit['qualified']!=(error<1e-6)or fit['curves']!=fit['native_outputs']:raise ValueError('invalid fitted-curve qualification row')
    clipping=[json.loads(s)for s in re.findall(r'SURFACE_OFFSET_CLIPPING (\{[^\n]+\})',log)]
    clipping_request=json.loads((HERE/'fixtures/surface_curve_offset_clipping.json').read_text())
    if len(clipping)!=5 or [x['id']for x in clipping]!=[x['id']for x in clipping_request['operations']]:raise ValueError('one complete closed-gap replay is required')
    for clip in clipping:
        error=clip['maximum_bidirectional_sample_error']
        if not math.isfinite(error)or error<0 or clip['qualified']!=(error<1e-6):raise ValueError('invalid closed-gap qualification row')
    q=json.loads((HERE/'fixtures/surface_curve_offset_verify.json').read_text());r=json.loads((HERE/'observations/surface_curve_offset_verify.json').read_text());verified=[]
    for op,row in zip(q['operations'],r['results']):
        if op['id']!=row['id']or row.get('error'):raise ValueError('invalid independent SDK result')
        source=next(x for x in rows if x['id']==op['id'].removesuffix('_verify'))
        native=row['value']['sample_distances'];candidates=source['candidates']
        if len(native)!=7 or len(candidates)!=7 or len(op['samples'])!=7:raise ValueError('incomplete independent queries')
        point_gap=max(math.dist(a,b['point'])for a,b in zip(op['samples'],candidates))
        if point_gap>1e-10:raise ValueError('candidate points changed; refresh the SDK verification capture')
        delta=max(abs(a-b['nearest_distance'])for a,b in zip(native,candidates))
        verified.append(dict(id=op['id'],queries=len(native),maximum_sdk_distance=max(native),maximum_closest_solver_difference=delta,maximum_query_point_difference=point_gap))
    report=dict(model_epsilon=1e-6,full_native_parity=False,command_registered=False,recipes=len(rows),stations=168,qualified_recipes=sum(x['qualified']for x in rows),unqualified_recipes=[x['id']for x in rows if not x['qualified']],maximum_station_difference=max(x['maximum_error']for x in rows),independent_sdk_queries=sum(x['queries']for x in verified),cases=[{k:v for k,v in x.items()if k!='candidates'}for x in rows],independent_closest_points=verified,limits=['Geodesic point foundation only; OffsetCrvOnSrf is not registered.','Station comparisons do not certify the complete offset locus.','Native discrepancies remain subject to the unchanged 1e-6 qualification gate.','Natural boundaries, poles and non-C1 chart transitions do not return partial endpoints.'])
    report['limits'][0]='Geodesic point and fitted-curve foundation; OffsetCrvOnSrf is not registered.'
    curves=[x for x in fits if x['status']=='curve']
    report['fitted_curves']=dict(recipes=24,curves=sum(x['curves']for x in fits),empty_recipes=sum(x['status']=='empty'for x in fits),natural_domain_rejections=0,qualified_recipes=sum(x['qualified']for x in fits),unqualified_recipes=[x['id']for x in fits if not x['qualified']],bidirectional_samples_per_curve=130,maximum_sample_difference=max(x['maximum_bidirectional_sample_error']for x in curves),cases=fits,limits=['Complete curve output and UV boundary crossings are fitted; comparisons and topology checks remain sampled.','The negative 0.25 warped offset now clips to both natural U boundaries; its original numerical discrepancy remains.','Trimmed faces, general closed-gap reconnection, corners and cut-locus removal still require qualification.'])
    report['closed_gap_cases']=dict(recipes=5,qualified_recipes=sum(x['qualified']for x in clipping),bidirectional_samples_per_curve=258,maximum_sample_difference=max(x['maximum_bidirectional_sample_error']for x in clipping),cases=clipping,limits=['Open mode retains separate natural-domain portions.','Captured closed planar SDK offsets reconnect retained portions with parabolic spans, which can leave the natural domain.','ConnectClosed is currently qualified for affine planar patches; other clipped closed-source gaps fail explicitly.'])
    report['sha256']={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in SOURCES}
    return report
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('replay',type=Path);args=parser.parse_args()
    (ROOT/'docs/surface-geodesic-qualification.json').write_text(json.dumps(summarize(args.replay.read_text()),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
