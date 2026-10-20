"""Summarize station qualification without changing the model parity gate."""
import argparse,hashlib,json,math,re
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
SOURCES=['crates/viboceros-geometry/src/surface_geodesic.rs','crates/viboceros-geometry/src/surface_geodesic/tests.rs','crates/viboceros-geometry/src/surface_curve_offset.rs','crates/viboceros-geometry/src/surface_curve_offset/tests.rs','crates/viboceros-geometry/src/surface_curve_offset/clip.rs','crates/viboceros-geometry/src/surface_curve_offset/clip/tests.rs','crates/viboceros-geometry/src/surface_curve_offset/clip/closed_gap.rs','crates/viboceros-geometry/src/nurbs_surface/offset_extension.rs','crates/viboceros-geometry/src/morph/curve_fit.rs','crates/viboceros-geometry/src/integration/cash_karp.rs','crates/viboceros-geometry/tests/surface_geodesic_native.rs','tools/rhino_oracle/fixtures/surface_curve_offset.json','tools/rhino_oracle/observations/surface_curve_offset.json','tools/rhino_oracle/fixtures/surface_curve_offset_verify.json','tools/rhino_oracle/observations/surface_curve_offset_verify.json','tools/rhino_oracle/fixtures/surface_curve_offset_clipping.json','tools/rhino_oracle/observations/surface_curve_offset_clipping.json','tools/rhino_oracle/report_surface_geodesic.py']
SOURCES += ['crates/viboceros-command/src/surface_curve_offset.rs', 'crates/viboceros-command/src/surface_curve_offset/tests.rs', 'crates/viboceros-command/tests/support/surface_curve_offset.rs', 'crates/viboceros-command/tests/surface_curve_offset_native.rs', 'src/app/surface_curve_offset.rs', 'src/app/tests/surface_curve_offset.rs', 'tools/rhino_oracle/fixtures/surface_offset_command.json', 'tools/rhino_oracle/observations/surface_offset_command.json']
SOURCES += ['crates/viboceros-geometry/src/surface_curve_offset/through_point.rs', 'crates/viboceros-command/src/surface_curve_offset/through_point.rs', 'crates/viboceros-geometry/src/brep.rs', 'tools/rhino_oracle/fixtures/surface_offset_through_point.json', 'tools/rhino_oracle/observations/surface_offset_through_point.json']
SOURCES += ['crates/viboceros-geometry/src/spline_collocation/axis.rs', 'crates/viboceros-geometry/src/morph.rs', 'tools/rhino_oracle/fixtures/surface_offset_through_verify.json', 'tools/rhino_oracle/observations/surface_offset_through_verify.json', 'tools/rhino_oracle/generate_surface_through_verify.py']
SOURCES += ['src/app.rs', 'tools/rhino_oracle/fixtures/surface_offset_select_point.json', 'tools/rhino_oracle/observations/surface_offset_select_point.json', 'tools/rhino_oracle/surface_offset_command_probe.py', 'tools/rhino_oracle/generate_surface_offset_command.py', 'tools/rhino_oracle/test_surface_offset_command.py']
SOURCES += ['crates/viboceros-geometry/src/lib.rs', 'crates/viboceros-geometry/src/surface_curve_offset/extend.rs', 'tools/rhino_oracle/fixtures/surface_offset_curved_ends.json', 'tools/rhino_oracle/observations/surface_offset_curved_ends.json', 'tools/rhino_oracle/fixtures/surface_curve_offset_extensions.json', 'tools/rhino_oracle/observations/surface_curve_offset_extensions.json', 'tools/rhino_oracle/surface_curve_offset_probe.py', 'tools/rhino_oracle/generate_surface_curve_offset.py', 'tools/rhino_oracle/structural_sdk.py']
SOURCES += ['crates/viboceros-geometry/src/surface_pullback.rs', 'crates/viboceros-geometry/src/surface_pullback/interpolation.rs', 'crates/viboceros-geometry/src/surface_pullback/periodic.rs', 'crates/viboceros-geometry/src/surface_pullback/periodic/tests.rs', 'crates/viboceros-geometry/src/surface_curve_offset/normal_extend.rs', 'crates/viboceros-geometry/src/surface_curve_offset/normal_extend/tests.rs', 'tools/rhino_oracle/test_surface_curve_offset.py']
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
    report=dict(model_epsilon=1e-6,full_native_parity=False,command_registered=True,recipes=len(rows),stations=168,qualified_recipes=sum(x['qualified']for x in rows),unqualified_recipes=[x['id']for x in rows if not x['qualified']],maximum_station_difference=max(x['maximum_error']for x in rows),independent_sdk_queries=sum(x['queries']for x in verified),cases=[{k:v for k,v in x.items()if k!='candidates'}for x in rows],independent_closest_points=verified,limits=['OffsetCrvOnSrf distance and ThroughPoint modes are registered; full native parity remains pending.','Station comparisons do not certify the complete offset locus.','Native discrepancies remain subject to the unchanged 1e-6 qualification gate.','Natural boundaries, poles and non-C1 chart transitions do not return partial endpoints.'])
    curves=[x for x in fits if x['status']=='curve']
    report['fitted_curves']=dict(recipes=24,curves=sum(x['curves']for x in fits),empty_recipes=sum(x['status']=='empty'for x in fits),natural_domain_rejections=0,qualified_recipes=sum(x['qualified']for x in fits),unqualified_recipes=[x['id']for x in fits if not x['qualified']],bidirectional_samples_per_curve=130,maximum_sample_difference=max(x['maximum_bidirectional_sample_error']for x in curves),cases=fits,limits=['Complete curve output and UV boundary crossings are fitted; comparisons and topology checks remain sampled.','The negative 0.25 warped offset now clips to both natural U boundaries; its original numerical discrepancy remains.','General trimmed-face parity, closed-gap reconnection, corners and cut-locus removal still require qualification.'])
    report['closed_gap_cases']=dict(recipes=5,qualified_recipes=sum(x['qualified']for x in clipping),bidirectional_samples_per_curve=258,maximum_sample_difference=max(x['maximum_bidirectional_sample_error']for x in clipping),cases=clipping,limits=['Open mode retains separate natural-domain portions.','Captured closed planar SDK offsets reconnect retained portions with parabolic spans, which can leave the natural domain.','ConnectClosed is currently qualified for affine planar patches; other clipped closed-source gaps fail explicitly.'])
    commands=[json.loads(s)for s in re.findall(r'SURFACE_OFFSET_COMMAND (\{[^\n]+\})',log)]
    command_request=json.loads((HERE/'fixtures/surface_offset_command.json').read_text())
    command_response=json.loads((HERE/'observations/surface_offset_command.json').read_text())
    if len(commands)!=8 or [x['id']for x in commands]!=[x['id']for x in command_request['operations']]:raise ValueError('one complete public command replay is required')
    for row, native in zip(commands,command_response['results']):
        error=row['maximum_bidirectional_sample_error']
        if not math.isfinite(error)or error<0 or row['qualified']!=(error<1e-6)or row['outputs']!=len(native['value']['after_script'])-2:raise ValueError('invalid command qualification row')
    report['command_cases']=dict(recipes=8,qualified_recipes=sum(x['qualified']for x in commands),unqualified_recipes=[x['id']for x in commands if not x['qualified']],bidirectional_samples_per_curve=258,maximum_sample_difference=max(x['maximum_bidirectional_sample_error']for x in commands),cases=commands,limits=['Planar open curves extend along endpoint tangents to natural boundaries, then clip against face loops.','Number advances the complete curve before trimming each copy; hole splits do not duplicate later copies.','Source selection survives; outputs use the current layer and remain unselected.','Full curved-surface endpoint-continuation parity remains pending.'])
    through=[json.loads(s)for s in re.findall(r'SURFACE_OFFSET_THROUGH_POINT (\{[^\n]+\})',log)]
    through_request=json.loads((HERE/'fixtures/surface_offset_through_point.json').read_text())
    through_response=json.loads((HERE/'observations/surface_offset_through_point.json').read_text())
    if len(through)!=16 or [x['id']for x in through]!=[x['id']for x in through_request['operations']]:raise ValueError('one complete ThroughPoint replay is required')
    for row,native in zip(through,through_response['results']):
        error=row['maximum_bidirectional_sample_error']
        if not math.isfinite(error)or error<0 or row['qualified']!=(error<1e-6)or row['outputs']!=len(native['value']['after_script'])-2:raise ValueError('invalid ThroughPoint qualification row')
    report['through_point_cases']=dict(recipes=16,qualified_recipes=sum(x['qualified']for x in through),unqualified_recipes=[x['id']for x in through if not x['qualified']],bidirectional_samples_per_curve=258,maximum_sample_difference=max(x['maximum_bidirectional_sample_error']for x in through),cases=through,limits=['Point input constrains to the selected trimmed face; its final curve must pass through that constrained point within model tolerance.','Repeated points offset the original source once; Number and Flip are ignored. Accepted points share one history entry and survive cancellation.','Captured B-rep topology is preserved. Distinct-edge nearest-point ties follow topology index rather than world axes.','The bounded normal-family inverse has no global cut-locus or root-completeness certificate. Full curved-surface endpoint-continuation parity remains pending.'])
    verification=json.loads((HERE/'observations/surface_offset_through_verify.json').read_text())
    if len(verification['results'])!=9 or any(r.get('error')for r in verification['results']):raise ValueError('nine independent ThroughPoint SDK queries are required')
    report['through_point_sdk_queries']=[dict(id=r['id'],distance=r['value']['distance'])for r in verification['results']]
    selected=[json.loads(s)for s in re.findall(r'SURFACE_OFFSET_SELECT_GETTER (\{[^\n]+\})',log)]
    scripted=[json.loads(s)for s in re.findall(r'SURFACE_OFFSET_SELECT_POINT (\{[^\n]+\})',log)]
    selected_request=json.loads((HERE/'fixtures/surface_offset_select_point.json').read_text())
    selected_response=json.loads((HERE/'observations/surface_offset_select_point.json').read_text())
    if len(selected)!=8 or [x['id']for x in selected]!=[x['id']for x in selected_request['operations']]:raise ValueError('one complete Select getter replay is required')
    if [x['id']for x in scripted]!=[r['id']for r in selected_response['results']if r['value']['success']]:raise ValueError('one complete scripted Select replay is required')
    for row,native in zip(selected,selected_response['results']):
        outputs=len(native['value']['after_script'])-len(native['value']['before'])
        for actual in [row]+[x for x in scripted if x['id']==row['id']]:
            error=actual['maximum_bidirectional_sample_error']
            if not math.isfinite(error)or error<0 or actual['qualified']!=(error<1e-6)or actual['outputs']!=outputs:raise ValueError('invalid Select qualification row')
    report['point_selection_cases']=dict(recipes=8,scripted_recipes=len(scripted),output_recipes=sum(x['outputs']>0 for x in selected),qualified_recipes=sum(x['qualified']for x in selected),maximum_sample_difference=max(x['maximum_bidirectional_sample_error']for x in selected),bidirectional_samples_per_curve=258,cases=selected,limits=['Select accepts ordered point objects at the first ThroughPoint getter and finishes on Enter.','On-source points are skipped; distinct point objects at the same offset are retained.','Selection is released; cancellation, empty selection and all-on-source batches create no geometry.','All nonzero results are prepared before one atomic commit; unresolved geometry does not commit a partial batch.','Full curved-surface endpoint-continuation parity remains pending.'])
    curved=[json.loads(s)for s in re.findall(r'SURFACE_OFFSET_CURVED_ENDS (\{[^\n]+\})',log)]
    curved_request=json.loads((HERE/'fixtures/surface_offset_curved_ends.json').read_text())
    curved_response=json.loads((HERE/'observations/surface_offset_curved_ends.json').read_text())
    if len(curved)!=8 or [x['id']for x in curved]!=[x['id']for x in curved_request['operations']]:raise ValueError('one complete curved-end replay is required')
    for row,native in zip(curved,curved_response['results']):
        if row['native_outputs']!=len(native['value']['after_script'])-2:raise ValueError('incomplete native curved-end output')
        error=row['maximum_bidirectional_sample_error']
        if row['status']!='curve' or not math.isfinite(error) or error<0 or row['qualified']!=(error<1e-6) or row['outputs']!=row['native_outputs']:raise ValueError('invalid curved-end qualification row')
    report['curved_end_cases']=dict(recipes=8,qualified_recipes=sum(x['qualified']for x in curved),unqualified_recipes=[x['id']for x in curved if not x['qualified']],rejected_recipes=[x['id']for x in curved if x['status']=='rejected'],maximum_sample_difference=max(x['maximum_bidirectional_sample_error']for x in curved if x['status']=='curve'),bidirectional_samples_per_curve=258,cases=curved,limits=['Endpoint normal sections reuse exact surface/plane intersections, with physical and UV normalization. Unsupported sections fail before document edits.','Periodic pullbacks and every restored or trimmed output receive complete surface-image proofs. The command emits chart portions while Number advances the complete curve.','Captured sources share preparation across point picks and option changes. Curved offset endpoint directions follow the transverse geodesic family.','Native numerical discrepancies remain separate from the unchanged model gate. This does not certify the complete offset locus or all periodic topology.'])
    extension=json.loads((HERE/'observations/surface_curve_offset_extensions.json').read_text())
    if len(extension['results'])!=8 or any(r.get('error')or not r['value']['source_unchanged']for r in extension['results']):raise ValueError('eight immutable independent extension SDK scenes are required')
    report['extension_sdk_cases']=[dict(id=r['id'],extended_source_controls=len(r['value']['extended_source']['control_points']),offsets_after_source_extension=len(r['value']['offset_extended_source']),extended_offset_results=len(r['value']['extended_offsets']))for r in extension['results']]
    normal=[json.loads(s)for s in re.findall(r'SURFACE_NORMAL_EXTENSION (\{[^\n]+\})',log)]
    if len(normal)!=8 or [x['id']for x in normal]!=[x['id']for x in extension['results']]:raise ValueError('one complete normal-section source replay is required')
    for row in normal:
        error=row['maximum_bidirectional_sample_error'];endpoint=row['maximum_endpoint_difference']
        if not math.isfinite(error) or error<0 or row['qualified']!=(error<1e-6) or not math.isfinite(endpoint) or endpoint<0 or endpoint>=1e-7:raise ValueError('invalid normal-section qualification row')
    diagonal=next(x for x in normal if x['id']=='curved_diagonal')
    if not 2e-6<diagonal['native_maximum_cylinder_residual']<4e-6 or not 2.4e-3<diagonal['native_offset_start_difference']<2.5e-3:raise ValueError('missing independent cylinder discrepancy checks')
    report['normal_extension_cases']=dict(recipes=8,qualified_recipes=sum(x['qualified']for x in normal),unqualified_recipes=[x['id']for x in normal if not x['qualified']],bidirectional_samples_per_curve=130,maximum_sample_difference=max(x['maximum_bidirectional_sample_error']for x in normal),maximum_endpoint_difference=max(x['maximum_endpoint_difference']for x in normal),cases=normal,limits=['Seven source continuations meet the unchanged 1e-6 sampled gate; all eight endpoint pairs agree within 1e-7.','The captured cylinder diagonal continuation is itself off the exact cylinder by about 2.9e-6. Its SDK offset begins about 2.45e-3 before the analytic normal-family endpoint.','Section orientation and half-section limits are inferred from public SDK evidence. General section support and full native parity remain pending.'])
    report['sha256']={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in SOURCES}
    return report
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('replay',type=Path);args=parser.parse_args()
    (ROOT/'docs/surface-geodesic-qualification.json').write_text(json.dumps(summarize(args.replay.read_text()),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
