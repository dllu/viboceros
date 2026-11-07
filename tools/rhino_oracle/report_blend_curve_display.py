"""Pin live curvature, transient analysis and continuation-default evidence."""
import argparse,gzip,hashlib,json,math,re,subprocess
from .blend_curve_display_evidence import collect,BASELINE
from .blend_curve_evidence import command_result
from .check_blend_curve_display import ROOT,PROOF,commands
from .report_arc_blend import records
FIELDS=['maximum_coefficient_and_sample_error','maximum_native_sample_error','maximum_normalized_derivative_error','maximum_curvature_vector_error','maximum_scaled_curvature_tip_error','maximum_absolute_derivative_error','maximum_parameter_domain_error','maximum_parameter_domain_gate_ratio']

def comparisons(proof):
 workflows,captures,counts,paths=collect(proof);cases={key:dict(id=key)for key in workflows}
 for mode in ['portable','native']:
  viewport=gzip.decompress((proof/('graph-viewport-'+mode+'.log.gz')).read_bytes()).decode()
  if 'blend_combs_render_in_each_view_and_mode_without_modifying_the_document'not in viewport or 'test result: ok. 3 passed; 0 failed;'not in viewport:raise ValueError('missing blend viewport coverage')
  for stage in ['command','ui']:
   log='display-'+stage+'-'+mode;marker='BLEND_CURVE_DISPLAY_'+stage.upper()
   text=gzip.decompress((proof/(log+'.log.gz')).read_bytes()).decode()
   required=['native_continuity_defaults_distinguish_preselection_from_preview_controls','curvature_enable_and_cache_are_readonly_and_geometry_changes_invalidate_samples']if stage=='command'else['cached_hover_combs_follow_candidates_and_cancel_restores_the_accepted_preview']
   if any(name not in text for name in required):raise ValueError('missing display cache or continuation coverage')
   rows=records(proof,log,marker,workflows,FIELDS[0],archives=True)
   for field in FIELDS[1:5]:records(proof,log,marker,workflows,field)
   for row in rows:
    value=workflows[row['id']];accepted=command_result(value)=='Nothing'
    derivatives=sum(len(o.get('derivative_samples',[]))for phase in ['before','after']for o in value[phase]);curvatures=sum(len(o.get('curvature_samples',[]))for phase in ['before','after']for o in value[phase])
    if row['accepted']is not accepted or row['native_stations']!=(33 if accepted else 0)or row['derivative_stations']!=derivatives or row['curvature_stations']!=curvatures:raise ValueError('display replay classification changed')
    if any(type(row[f])not in[int,float]or not math.isfinite(row[f])or row[f]<0 for f in FIELDS[5:])or row[FIELDS[-1]]>1:raise ValueError('invalid display domain or derivative diagnostic')
    spans=[abs(o['curve']['domain'][1]-o['curve']['domain'][0])for phase in ['before','after','undo','redo']for o in value[phase]if 'curve'in o];bound=max(2e-6,1e-8*max(spans,default=0.))
    if row[FIELDS[-2]]>bound or row[FIELDS[-1]]+1e-12<row[FIELDS[-2]]/bound:raise ValueError('display fractional domain gate failed')
    for field in FIELDS:cases[row['id']][mode+'_'+stage+'_'+field]=row[field]
 return cases,captures,counts,paths

def build(proof,revision):
 if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full display implementation revision required')
 git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
 git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
 checks=json.loads((proof/'checks.json').read_bytes())
 if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete or failed display checks')
 cases,captures,counts,paths=comparisons(proof)
 changed=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
 required={'crates/viboceros-command/src/blend_curve/display.rs','src/app/blend_curve/display.rs','src/viewport/curvature_graph.rs','src/app/curvature_graph.rs'}
 if not required.issubset(changed):raise ValueError('missing live display implementation')
 maximum=lambda field:max(c[mode+'_'+stage+'_'+field]for c in cases.values()for mode in ['portable','native']for stage in ['command','ui'])
 sha=lambda raw:hashlib.sha256(raw).hexdigest()
 return dict(scope='BlendCrv DisplayCurvature enables temporary source and candidate combs, shares scale/density/color controls, follows edits, restores existing graphs, and inherits active analysis. Continuity changes in a ready preview persist; preselection overrides do not.',
  baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,native_published_observations=27,matched_workflows_per_build=len(cases),retained_diagnostic_observations=1,retained_interrupted_captures=1,**counts,
  model_epsilon=1e-6,normalized_derivative_relative_epsilon=1e-6,parameter_domain_absolute_floor=2e-6,parameter_domain_relative_tolerance=1e-8,
  maximum_geometry_difference=max(maximum(FIELDS[0]),maximum(FIELDS[1])),maximum_normalized_derivative_relative_difference=maximum(FIELDS[2]),maximum_curvature_vector_difference=maximum(FIELDS[3]),maximum_scaled_curvature_tip_difference=maximum(FIELDS[4]),maximum_absolute_derivative_difference=maximum(FIELDS[5]),maximum_parameter_domain_difference=maximum(FIELDS[6]),maximum_parameter_domain_gate_ratio=maximum(FIELDS[7]),
  archive_round_trips_per_build=2*len(cases),viewport_combinations_per_build=12,
  input_evidence='Prescribed sources, public pick rays, initial getter options and explicitly prescribed analysis states construct replays. Result geometry, public CurvatureAt evaluations and viewport pixels are expectations only. Two separate stateful sequences replay continuity preferences without reseeding between commands.',
  construction='The existing Rust curvature-comb sampler and usd-brep extraction serve temporary source snapshots and each candidate. Geometry and differential caches survive scale/color changes. The shared viewport painter clips hairs and outlines; no temporary objects or undo entries are created.',
  execution='Rhino captures ran on private Xvfb displays; all application and viewport tests were windowless.',
  limits=['A pilot sent one unnecessary final Enter after Native had already accepted. Its incomplete input is a diagnostic, repeated successfully. An overlapping unrelated fixture opened a selection menu; the owned process was interrupted with no published observations, then all recipes were repeated with separated geometry and explicit display setup.',
   'Bitmap evidence establishes the observed Native composition and lifecycle, not pixel equality. Full graph-panel mouse trajectories, arbitrary depth/occlusion, singular extremes and comparative performance remain unqualified. Sampling is bounded at 200000 stations; existing G3/G4 mathematical-continuity and approximate-domain limits remain.'],
  cases=[cases[k]for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],
  implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in changed},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(paths)})

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
 (ROOT/'docs/blend-curve-display-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
