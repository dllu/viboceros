"""Pin curved-face G3/G4 construction and third-derivative qualification."""
import argparse
import gzip
import hashlib
import json
import math
import re
import subprocess
from .blend_curve_surface_high_evidence import collect,BASELINE,fourth_policy,length_diagnostics
from .blend_curve_surface_policy import characterize
from .blend_curve_evidence import command_result
from .check_blend_curve_surface_high import ROOT,PROOF,commands
from .report_arc_blend import records
FIELDS=['maximum_coefficient_and_sample_error','maximum_native_sample_error','maximum_normalized_derivative_error','maximum_absolute_derivative_error','maximum_parameter_domain_error','maximum_parameter_domain_gate_ratio']
IGNORED_KERNEL={
 'curve_fit::g1::correspondence::linear::tests::saved_elevated_curve_timings',
 'curve_join::tests::search::endpoint_search_benchmark','curve_join::tests::seeded::seeded_join_benchmark',
 'loft::refit::preparation::tests::export_legacy_preparation_baseline',
 'nurbs_surface::evaluate::grid::tests::benchmark_surface_grid_evaluation',
 'nurbs_surface::evaluate::normal::tests::regular_normal_query_microbenchmark',
 'point_cloud::tests::coincident_projected_index_query_benchmark','point_cloud::tests::oblique_frame_query_benchmark',
 'point_cloud::tests::projected_index_query_benchmark','transform::orientation::tests::benchmark_diagonal_orientation',
}
def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete or failed checks')
    workflows,new,captures,counts,evidence=collect(proof)
    diagnostics=length_diagnostics(new,proof)
    cases={k:dict(id=k,domain_qualified=k not in diagnostics)for k in workflows}
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            log,marker='surface-high-'+stage+'-'+mode,'BLEND_CURVE_SURFACE_HIGH_'+stage.upper()
            rows=records(proof,log,marker,workflows,FIELDS[0],archives=True)
            for field in FIELDS[1:3]:records(proof,log,marker,workflows,field)
            for row in rows:
                v=workflows[row['id']];accepted=command_result(v)=='Nothing'
                count=sum(len(o.get('derivative_samples',[]))for phase in ['before','after']for o in v[phase])
                if row['native_stations']!=(33 if accepted else 0)or row['accepted']is not accepted or row['derivative_stations']!=count:raise ValueError('higher surface replay classification changed')
                absolute,domain,ratio=[row[f]for f in FIELDS[3:]]
                if any(type(x)not in[int,float]or not math.isfinite(x)or x<0 for x in[absolute,domain,ratio]):raise ValueError('invalid higher surface replay diagnostic')
                spans=[abs(o['curve']['domain'][1]-o['curve']['domain'][0])for phase in ['before','after','undo','redo']for o in v[phase]if 'curve'in o];bound=max(2e-6,1e-8*max(spans,default=0.))
                qualified=row['id']not in diagnostics
                if row.get('domain_qualified')is not qualified:raise ValueError('higher surface domain classification changed')
                independent=row.get('maximum_independent_length_error')
                if type(independent)not in[int,float]or not math.isfinite(independent)or not 0<=independent<=1e-7:raise ValueError('independent long-handle length gate failed')
                if ratio+1e-12<domain/bound:raise ValueError('higher surface fractional domain diagnostic is inconsistent')
                if qualified:
                    if ratio>1 or domain>bound:raise ValueError('higher surface fractional domain gate failed')
                elif ratio<=1 or domain<=bound:raise ValueError('lost observed long-handle domain mismatch')
                cases[row['id']][mode+'_'+stage+'_maximum_independent_length_error']=independent
                for field in FIELDS:cases[row['id']][mode+'_'+stage+'_'+field]=row[field]
    partials=records(proof,'surface-third-native','SURFACE_THIRD_NATIVE',new,'maximum_point_error')
    for row in partials:
        value=row['maximum_partial_relative_error'];v=new[row['id']]
        expected=sum(len(q['faces'])for c in v['input_trace'][0]['clicks']for q in c.get('edge_pick_context',{}).get('candidates',[]))
        absolute=row['maximum_partial_absolute_error']
        if (row['stations']!=expected or expected==0 or type(value)not in[int,float]or not math.isfinite(value)or not 0<=value<=1e-8
                or type(absolute)not in[int,float]or not math.isfinite(absolute)or absolute<0):raise ValueError('third surface partial replay gate failed')
    kernel=gzip.decompress((proof/'surface-third-kernel.log.gz').read_bytes()).decode()
    count=re.search(r'^running (\d+) tests$',kernel,re.M)
    result=re.search(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',kernel)
    ignored=re.findall(r'^test (\S+) \.\.\. ignored(?:, (.*))?$',kernel,re.M)
    if (count is None or int(count[1])<2862 or result is None or int(result[1])<2852
            or int(count[1])!=int(result[1])+int(result[2])or int(result[2])!=10
            or {name for name,_ in ignored}!=IGNORED_KERNEL):raise ValueError('missing full geometry kernel regression')
    paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required={'crates/viboceros-geometry/src/nurbs_surface/evaluate/third.rs','crates/viboceros-geometry/src/nurbs_surface/evaluate/third/exact.rs','crates/viboceros-geometry/src/surface_directional.rs','crates/viboceros-command/src/blend_curve/surface.rs','crates/viboceros-command/src/blend_curve/options.rs','src/app/tests/blend_curve_surface_high.rs'}
    if not required.issubset(paths):raise ValueError('missing curved-face high-order implementation')
    maximum=lambda field,qualified=False:max(c[mode+'_'+stage+'_'+field]for c in cases.values()if not qualified or c['domain_qualified']for mode in ['portable','native']for stage in ['command','ui'])
    sha=lambda b:hashlib.sha256(b).hexdigest();policy=characterize(new)
    return dict(scope='Curved-face BlendCrv G3/G4 uses analytic third surface partials and normal projections of the normalized straight-UV-line jet. Numeric angles, contact/shape/Shift edits, mixed sources, joins, charts, nearly planar G2/G3/G4 faces with long handles and archive/history workflows are checked.',
        baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,model_epsilon=1e-6,normalized_derivative_relative_epsilon=1e-6,surface_partial_relative_epsilon=1e-8,parameter_domain_absolute_floor=2e-6,parameter_domain_relative_tolerance=1e-8,input_recipe_pick_tolerance_ulps=8,
        native_observations=42,new_native_observations=40,promoted_native_observations=2,matched_document_workflows_per_build=39,accepted_document_workflows=38,geometry_and_history_replays_per_build=42,accepted_geometry_replays=41,domain_diagnostic_workflows=3,native_command_results=dict(Nothing=41,Cancel=1),native_success_boolean=False,
        **counts,source_identity_checked=True,archive_round_trips_per_build=84,geometry_kernel_tests=int(count[1]),geometry_kernel_tests_passed=int(result[1]),geometry_kernel_tests_ignored=[dict(name=name,reason=reason or 'manual legacy preparation export')for name,reason in ignored],
        maximum_geometry_difference=max(maximum(FIELDS[0]),maximum(FIELDS[1])),maximum_normalized_derivative_relative_difference=maximum(FIELDS[2]),maximum_normalized_derivative_absolute_difference=maximum(FIELDS[3]),maximum_parameter_domain_difference=maximum(FIELDS[4]),maximum_parameter_domain_gate_ratio=maximum(FIELDS[5]),
        maximum_qualified_parameter_domain_difference=maximum(FIELDS[4],True),maximum_qualified_parameter_domain_gate_ratio=maximum(FIELDS[5],True),maximum_independent_length_difference=maximum('maximum_independent_length_error'),parameter_domain_diagnostics=[dict(id=key,**value)for key,value in diagnostics.items()],
        maximum_surface_point_difference=max(r['maximum_point_error']for r in partials),maximum_surface_partial_relative_difference=max(r['maximum_partial_relative_error']for r in partials),
        maximum_parameter_line_policy_difference=max(r['parameter_line_error']for r in policy),maximum_normal_section_policy_difference=max(r['normal_section_error']for r in policy),fourth_order_policy=fourth_policy(new),
        retained_failed_capture_attempts=1,retained_failed_planarity_regressions=2,retained_lint_attempts=1,cases=[cases[k]for k in sorted(cases)],surface_partial_replays=partials,captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],
        construction='Active-span homogeneous derivative nets through total order three feed a rational quotient recurrence. Signed cancellation, continuation and range loss recover exactly from original binary64 inputs before final rounding. Normalized parameter-line second/third targets project onto the face normal; Native G4 retains zero fourth derivative. Surface contact UVs update on contact movement and are shared across handle-length candidate copies. Native builds use usd-brep endpoint interpolation.',
        input_evidence='Prescribed source surfaces construct the Rust scenes. Public Surface.Evaluate third partials at passive picked UVs independently check the kernel. Native output coefficients and derivative samples are expectations only. CPython/IronPython boundary-sum picks differ by at most eight ulps; only these source-pick coordinates may vary, with all source definitions/options and recorded pixel/ray/face receipts retained.',
        execution='Private Xvfb captures and windowless egui checks; no desktop windows.',
        limits=['This reproduces the observed command policy. Straight-UV-line projection differs from a geometric normal section on nonorthogonal surfaces; mathematical G3/G4 contact on arbitrary reparameterized faces is not established. Native G4 uses a zero fourth target even on a measured nonzero-fourth source.',
                'Native checks cover regular isolated Bezier surface faces with polynomial/rational controls, curved boundaries, shifted/scaled parameter charts and Top/Perspective views. Signed weights, poles, one-sided third limits, rational continuation, overflowing intermediate nets and extreme chart scaling have independent kernel regressions.',
                'The retained failed private capture opened an ambiguous-edge menu. The successful retry rigidly rotates the prescribed surfaces and picks to separate their projected edges, and corrects the authored opposite-edge contact. General ambiguous picking, shared/seam faces, trimmed/closed boundaries, singular frames and Alt dragging remain unqualified.',
                'Thirty-nine workflows retain the earlier Native desired fractional-length gate. Three nearly planar 2000-unit-handle cases exceed it and remain explicit domain diagnostics. Their geometry, normalized derivatives, history and archives pass; Rust lengths agree within 1e-7 with independent integration of the captured Native curves. Exact Native approximate-domain parity, new public tight-length calibration, continuous-distance proof and comparative performance remain unqualified.'],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
    (ROOT/'docs/blend-curve-surface-high-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
