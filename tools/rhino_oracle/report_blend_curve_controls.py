"""Pin complete higher-control replays and retained diagnostic attempts."""
import argparse,gzip,hashlib,json,math,re,subprocess
from .blend_curve_controls_evidence import collect,BASELINE,DIAGNOSTICS,FAMILIES,DOMAIN,LENGTH
from .blend_curve_evidence import command_result
from .check_blend_curve_controls import ROOT,PROOF,commands
from .report_arc_blend import records
FIELDS=['maximum_coefficient_and_sample_error','maximum_native_sample_error','maximum_normalized_derivative_error','maximum_absolute_derivative_error','maximum_parameter_domain_error','maximum_parameter_domain_gate_ratio']
def build(proof,revision):
 if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full controls implementation revision required')
 git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
 git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
 checks=json.loads((proof/'checks.json').read_bytes())
 if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete or failed control checks')
 workflows,captures,counts,policy,evidence=collect(proof);cases={k:dict(id=k)for k in workflows}
 for mode in ['portable','native']:
  for stage in ['command','ui']:
   log,marker='controls-'+stage+'-'+mode,'BLEND_CURVE_CONTROLS_'+stage.upper()
   if stage=='ui':
    text=gzip.decompress((proof/(log+'.log.gz')).read_bytes()).decode()
    names=['higher_control_press_modifiers_and_numeric_cancel_preserve_accepted_preview','every_g4_shape_control_supports_a_held_pointer_drag']
    if 'test result: ok. 3 passed; 0 failed; 0 ignored;'not in text or any('app::tests::blend_curve_controls::interaction::'+n not in text for n in names):raise ValueError('missing higher-control windowless interaction coverage')
   rows=records(proof,log,marker,workflows,FIELDS[0],archives=True)
   for field in FIELDS[1:3]:records(proof,log,marker,workflows,field)
   for row in rows:
    v=workflows[row['id']];accepted=command_result(v)=='Nothing';count=sum(len(o.get('derivative_samples',[]))for phase in ['before','after']for o in v[phase])
    if row['native_stations']!=(33 if accepted else 0)or row['accepted']is not accepted or row['nominal_capture']is not (row['id']!=DOMAIN)or row['derivative_stations']!=count:raise ValueError('control replay classification changed')
    absolute,domain,ratio=[row[f]for f in FIELDS[3:]]
    if any(type(x)not in[int,float]or not math.isfinite(x)or x<0 for x in[absolute,domain,ratio]):raise ValueError('invalid control derivative/domain diagnostic')
    spans=[abs(o['curve']['domain'][1]-o['curve']['domain'][0])for phase in ['before','after','undo','redo']for o in v[phase]if 'curve'in o];bound=max(2e-6,1e-8*max(spans,default=0.))
    if row['id']==DOMAIN:
     if ratio<=1 or abs(domain-abs(policy['long_rational_domain']['native_domain_minus_length']))>1e-8:raise ValueError('control domain diagnostic classification changed')
    elif ratio>1 or domain>bound or ratio+1e-12<domain/bound:raise ValueError('control fractional domain gate failed')
    for field in FIELDS:cases[row['id']][mode+'_'+stage+'_'+field]=row[field]
 paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
 required={'crates/viboceros-command/src/blend_curve/controls.rs','crates/viboceros-command/src/blend_curve/shape.rs','crates/viboceros-geometry/src/curve_blend_pair/jets/spacing.rs','src/app/blend_curve/adjustment.rs','src/app/tests/blend_curve_controls.rs'}
 if not required.issubset(paths):raise ValueError('missing higher-control implementation')
 maximum=lambda field:max(c[mode+'_'+stage+'_'+field]for c in cases.values()for mode in ['portable','native']for stage in ['command','ui'])
 sha=lambda b:hashlib.sha256(b).hexdigest()
 return dict(scope='Independent G2-G4 shape controls, signed tangent gaps, primary resizing, symmetry, contact/angle changes, numeric direction confirmation, ordinary/surface/point sources and history.',
  baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,model_epsilon=1e-6,normalized_derivative_relative_epsilon=1e-6,parameter_domain_absolute_floor=2e-6,parameter_domain_relative_tolerance=1e-8,
  native_observations=sum(len(v)for v in FAMILIES.values())+1,matched_document_workflows_per_build=len(workflows)-1,accepted_document_workflows=sum(command_result(v)=='Nothing'for k,v in workflows.items()if k!=DOMAIN),geometry_and_history_replays_per_build=len(workflows),domain_diagnostic=DOMAIN,public_length_diagnostic=LENGTH,retained_numeric_timeout_diagnostics=sorted(DIAGNOSTICS),promoted_prior_secondary_observation='blend_curve_alt_selection/plain_alt_far_surface',
  **counts,archive_round_trips_per_build=2*len(workflows),source_identity_checked=True,retained_validation_attempts=6,
  maximum_geometry_difference=max(maximum(FIELDS[0]),maximum(FIELDS[1])),maximum_normalized_derivative_relative_difference=maximum(FIELDS[2]),maximum_normalized_derivative_absolute_difference=maximum(FIELDS[3]),maximum_parameter_domain_difference=maximum(FIELDS[4]),maximum_parameter_domain_gate_ratio=maximum(FIELDS[5]),
  maximum_qualified_parameter_domain_gate_ratio=max(c[mode+'_'+stage+'_'+FIELDS[5]]for key,c in cases.items()if key!=DOMAIN for mode in ['portable','native']for stage in ['command','ui']),
  cases=[cases[k]for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],construction_policy=policy,
  input_evidence='Source definitions and passive public input transforms, pixels, rays and acknowledgements construct every edit. Native output coefficients, domains, derivatives and samples are expectations only. Prompt fences carry no geometric input.',
  execution='Native captures use private Xvfb; egui and viewport tests run windowlessly.',
  limits=['Three numeric investigations timed out before direction confirmation and remain diagnostics, not matched cancellation workflows. All terminal statuses and frozen producer snapshots are retained.',
   'One long rational-source blend passes geometry/history checks but fails the unchanged domain gate. Independent quadrature agrees with Viboceros; Native command and default public SDK lengths share an approximate span that converges at tighter SDK tolerances. Its measurement-only repeat and the original failed check remain diagnostics.',
   'The measured G4 control response differs from mathematical scalar composition. The kernel exposes scalar-composition derivatives separately; this qualification follows the observed command policy.',
   'Complete Native drag/Escape trajectories, arbitrary occlusion/depth ties, shared/seam/trimmed/closed surface boundaries, singular frames, whole closed curve sources, curvature display, history reedit, continuous distance and comparative performance remain unqualified.'],
  implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
 (ROOT/'docs/blend-curve-controls-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
