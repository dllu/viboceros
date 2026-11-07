"""Pin whole closed-source geometry, option rejection and periodic padding."""
import argparse,gzip,hashlib,json,math,re,subprocess
from .blend_curve_closed_evidence import collect,BASELINE
from .blend_curve_evidence import command_result
from .check_blend_curve_closed import ROOT,PROOF,commands
from .report_arc_blend import records
FIELDS=['maximum_coefficient_and_sample_error','maximum_native_sample_error','maximum_normalized_derivative_error','maximum_absolute_derivative_error','maximum_parameter_domain_error','maximum_parameter_domain_gate_ratio','maximum_unused_outer_knot_difference']
def build(proof,revision):
 if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full closed implementation revision required')
 git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
 git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
 checks=json.loads((proof/'checks.json').read_bytes())
 if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete or failed closed checks')
 workflows,captures,counts,evidence=collect(proof);cases={k:dict(id=k)for k in workflows}
 for mode in ['portable','native']:
  for stage in ['command','ui']:
   log,marker='closed-'+stage+'-'+mode,'BLEND_CURVE_CLOSED_'+stage.upper()
   text=gzip.decompress((proof/(log+'.log.gz')).read_bytes()).decode()
   if stage=='ui':
    names=['windowless_closed_source_picks_contact_cancel_and_flip_keep_one_history_entry','a_closed_source_replaced_by_an_open_curve_resets_and_restores_natural_end_picking']
    if 'test result: ok. 3 passed; 0 failed; 0 ignored;'not in text or any('app::tests::blend_curve_closed::interaction::'+n not in text for n in names):raise ValueError('missing closed windowless interaction coverage')
   else:
    guard=[json.loads(line.split(' ',1)[1])for line in text.splitlines()if line.startswith('BLEND_CURVE_CLOSED_PADDING_GUARD ')]
    if len(guard)!=1 or set(guard[0])!={'geometry_error_for_unused_padding','unused_padding_difference','active_knot_error','control_error'}or any(type(x)not in[int,float]or not math.isfinite(x)or x<0 for x in guard[0].values()):raise ValueError('invalid periodic padding guard')
    if len(guard)!=1 or guard[0]['geometry_error_for_unused_padding']>1e-6 or guard[0]['unused_padding_difference']<=100 or guard[0]['active_knot_error']<=0.1 or guard[0]['control_error']<=0.01:raise ValueError('periodic padding guard changed')
   rows=records(proof,log,marker,workflows,FIELDS[0],archives=True)
   for field in FIELDS[1:3]:records(proof,log,marker,workflows,field)
   for row in rows:
    v=workflows[row['id']];accepted=command_result(v)=='Nothing';count=sum(len(o.get('derivative_samples',[]))for phase in ['before','after']for o in v[phase]);rejected=sum(line.startswith('Unknown command: _')for line in v['history'].splitlines())
    if row['native_stations']!=(33 if accepted else 0)or row['accepted']is not accepted or row['nominal_capture']is not True or row['derivative_stations']!=count or row['rejected_options']!=rejected:raise ValueError('closed replay classification changed')
    absolute,domain,ratio,padding=[row[f]for f in FIELDS[3:]]
    if any(type(x)not in[int,float]or not math.isfinite(x)or x<0 for x in[absolute,domain,ratio,padding]):raise ValueError('invalid closed derivative/domain/padding diagnostic')
    spans=[abs(o['curve']['domain'][1]-o['curve']['domain'][0])for phase in ['before','after','undo','redo']for o in v[phase]if 'curve'in o];bound=max(2e-6,1e-8*max(spans,default=0.))
    if ratio>1 or domain>bound or ratio+1e-12<domain/bound:raise ValueError('closed fractional domain gate failed')
    if padding!=(1. if '/closed_periodic_g'in row['id']else 0.):raise ValueError('periodic padding classification changed')
    for field in FIELDS:cases[row['id']][mode+'_'+stage+'_'+field]=row[field]
 paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
 required={'crates/viboceros-command/src/blend_curve/guide.rs','crates/viboceros-command/src/blend_curve/commit.rs','crates/viboceros-command/src/blend_curve/adjustment.rs','src/app/blend_curve/picking.rs','src/app/tests/blend_curve_closed.rs'}
 if not required.issubset(paths):raise ValueError('missing whole closed-source implementation')
 maximum=lambda field:max(c[mode+'_'+stage+'_'+field]for c in cases.values()for mode in ['portable','native']for stage in ['command','ui'])
 sha=lambda b:hashlib.sha256(b).hexdigest()
 return dict(scope='Whole closed BlendCrv sources use picked contacts in either start mode, retain their parents, and support G0-G4, seam contacts, flips, same-source references, shape retention and ordinary-source joins.',
  baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,model_epsilon=1e-6,normalized_derivative_relative_epsilon=1e-6,parameter_domain_absolute_floor=2e-6,parameter_domain_relative_tolerance=1e-8,
  native_observations=len(workflows),matched_document_workflows_per_build=len(workflows),accepted_document_workflows=sum(command_result(v)=='Nothing'for v in workflows.values()),**counts,archive_round_trips_per_build=2*len(workflows),source_identity_checked=True,
  maximum_geometry_difference=max(maximum(FIELDS[0]),maximum(FIELDS[1])),maximum_normalized_derivative_relative_difference=maximum(FIELDS[2]),maximum_normalized_derivative_absolute_difference=maximum(FIELDS[3]),maximum_parameter_domain_difference=maximum(FIELDS[4]),maximum_parameter_domain_gate_ratio=maximum(FIELDS[5]),maximum_unused_outer_knot_difference=maximum(FIELDS[6]),
  construction='Closed whole inputs always use the picked parameter and decreasing source direction at the start. Open inputs retain BlendStart semantics. Closed parents are never trimmed or consumed. Join needs an eligible open source; unavailable interactive Trim/Join entries reject atomically. Flip selects the nearest contact among references to the same object. Geometry uses the existing Rust jets and usd-brep interpolation.',
  input_evidence='Prescribed sources and passive public click points/rays/transforms construct every replay. The public Native NURBS setter consumes only inner knots; the fixture loader pads unused outer entries independently of expected output. Native output geometry is expectations only.',
  representation='For periodic parents, RhinoCommon exports unused outer knots by duplication while OpenNURBS reconstructs them by extrapolation on 3DM read. The two irrelevant entries have a separate diagnostic; every control, weight, active knot and sample remains in the strict geometry gate. Mutation tests require active-knot/control changes to fail.',
  cases=[cases[k]for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],retained_capture_attempts=2,retained_validation_attempts=6,
  execution='Private Xvfb Native captures and windowless egui tests; no desktop windows.',
  limits=['One eight-case pilot exited before publishing any observations. A local fixture setup error also preceded capture. Frozen producers, terminal receipts and logs remain; every pilot recipe repeats successfully.',
   'The corpus covers circles, a rational ellipse, a spatial periodic rational curve, a closed rational cubic with a nonsmooth seam, a closed polyline, point endpoints, same-source references and an ordinary curved join. General closed surface-boundary frame selection and arbitrary self-intersection/depth ties remain unqualified.',
   'Native drag/Escape trajectories, curvature display, history reediting, continuous distance and comparative performance remain unqualified. Earlier mathematical G3/G4 and long rational-domain diagnostics remain unchanged.'],
  implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
 (ROOT/'docs/blend-curve-closed-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
