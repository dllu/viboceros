"""Reconstruct projection-aware curve location and native fixed-line evidence."""
import argparse,gzip,hashlib,json,math,re,subprocess
from .check_line_pick_context import ROOT,PROOF,commands
from . import line_pick_context as recipes
HERE=ROOT/'tools/rhino_oracle'
BASELINE='b3d9a210e74bb9e600db77f12bfd2037d79f68ee'
FAMILIES={'line_pick_context':recipes.PILOT,'line_pick_context_holdouts':list(recipes.HOLDOUTS)}
def read(family):return json.loads(gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes()))
def collect_native(proof):
 sha=lambda b:hashlib.sha256(b).hexdigest();source=HERE/'fixtures/line_pick_context_sources.json'
 if json.loads(source.read_bytes())!=recipes.CASES:raise ValueError('prescribed sources changed')
 receipts=json.loads((proof/'captures.json').read_bytes())
 if [r['family']for r in receipts]!=list(FAMILIES):raise ValueError('capture inventory changed')
 evidence={p for p in proof.iterdir()if p.is_file()}|{source,ROOT/'docs/line-tangent-rays-validation.json',HERE/'observations/line_tangent_rays.json.gz'}
 matched={};captures=[];picks=0;maximum_contact_error=0.;world_errors={}
 for family,names in FAMILIES.items():
  fixture=HERE/('fixtures/'+family+'.json');path=HERE/('observations/'+family+'.json.gz');data=read(family)
  if [r['id']for r in data['results']]!=names or json.loads(fixture.read_bytes())!=recipes.request(names)or data['provenance']['request_sha256']!=sha(fixture.read_bytes()):raise ValueError('native coverage or request changed')
  evidence.update([fixture,path])
  for digest in data['provenance']['sources'].values():
   p=HERE/('observations/source_snapshots/'+digest+'.gz')
   if sha(gzip.decompress(p.read_bytes()))!=digest:raise ValueError('frozen producer changed')
   evidence.add(p)
  receipt=next(r for r in receipts if r['family']==family);log=gzip.decompress((proof/receipt['log']).read_bytes()).decode().splitlines()
  if receipt['exit_status']!=0 or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family]or log!=['Capturing '+str(len(names))+' cases: '+family,'Recorded '+family]:raise ValueError('capture incomplete')
  captures.append(dict(family=family,observations=len(names),raw_sha256=sha(gzip.decompress(path.read_bytes())),engine_version=data['engine_version']))
  for row in data['results']:
   k,v=row['id'],row['value'];case=recipes.CASES[k];trace=v['input_trace'][0]
   if k in matched or not v['success']or not v['returned_idle']or trace['errors']or trace['completed']!=len(case['actions'])or trace['expected']!=len(case['actions'])or v['prescribed_case']!=case:raise ValueError('incomplete native workflow')
   if len(v['after'])!=len(v['before'])+1 or v['after']!=v['redo']or [dict(o,selected=0)for o in v['before']]!=[dict(o,selected=0)for o in v['undo']]:raise ValueError('native history changed')
   if any(not o['native_valid']for o in v['after'])or [o['selected']for o in v['after']]!=[o['selected']for o in v['before']]+[0]:raise ValueError('native validity or selection changed')
   clicks=trace['clicks'];indices={i for i,a in enumerate(case['actions'])if isinstance(a,dict)}
   if len(clicks)!=2 or {c['action']for c in clicks}!=indices or trace['snap_settings']['planar']!=case['planar']:raise ValueError('physical input coverage changed')
   for c in clicks:
    action=case['actions'][c['action']];aim=action.get('pick',action.get('click'))
    if not c['motion_acknowledged']or not c['click_acknowledged']or len(c['ray'])!=2 or any(not math.isfinite(x)for p in c['ray']for x in p)or any(abs(a-b)>2*max(math.ulp(a),math.ulp(b))for a,b in zip(c['aim'],aim)):raise ValueError('physical pick receipt changed')
   click=next(c for c in clicks if c.get('curve')is not None);q=click['pick_context'];a,b=q['domain'];t=q['parameter'];matrix=q['transform'];point=q['point']
   if not q['success']or not a<=t<=b or len(point)!=3 or any(not math.isfinite(x)for x in point)or q['mode']!='Wireframe'or q['style']!='PointPick'or len(matrix)!=4 or any(len(row)!=4 or any(not math.isfinite(x)for x in row)for row in matrix):raise ValueError('invalid public pick context')
   h=point+[1.];clip=[sum(x*y for x,y in zip(row,h))for row in matrix]
   if clip[3]<=0 or max(abs(clip[0]/clip[3]),abs(clip[1]/clip[3]))>1.:raise ValueError('public source lies outside its pick frustum')
   points=[p['point']for p in v['after'][-1]['curve']['control_points']]
   contact=[(x+y)/2 for x,y in zip(*points)]if'BothSides'in case['actions']else points[0]
   error=math.dist(point,contact);maximum_contact_error=max(maximum_contact_error,error)
   if error>1e-6:raise ValueError('public pick does not match the native fixed contact')
   world_errors[k]=math.dist(click['sight_point'],point);matched[k]=v;picks+=2
 if len(matched)!=14 or picks!=28:raise ValueError('projected-pick coverage changed')
 legacy=next(r['value']for r in read('line_tangent_rays')['results']if r['id']=='ray_fixed_elevated');corrected=matched['pick_circle_perspective']
 if legacy['after'][-1]['curve']!=corrected['after'][-1]['curve']or legacy['input_trace'][0]['clicks'][0]['ray']!=corrected['input_trace'][0]['clicks'][0]['ray']:raise ValueError('legacy/corrected native input identity changed')
 return matched,captures,picks,maximum_contact_error,world_errors,evidence

def build(proof,revision):
 if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full implementation revision required')
 git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT);git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
 checks=json.loads((proof/'checks.json').read_bytes())
 if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete or failed checks')
 matched,captures,picks,native_error,world_errors,evidence=collect_native(proof);cases={k:dict(id=k)for k in matched}
 for mode in ['portable','native']:
  for stage in ['command','ui']:
   text=gzip.decompress((proof/('pick-'+stage+'-'+mode+'.log.gz')).read_bytes()).decode();rows=[json.loads(s)for s in re.findall(r'^LINE_PICK_CONTEXT_'+stage.upper()+r' (.+)$',text,re.M)]
   if len(rows)!=14 or {r['id']for r in rows}!=set(matched):raise ValueError('incomplete or duplicate projected-pick replay')
   for r in rows:
    error=r['maximum_coefficient_and_sample_error']
    if type(error)not in[int,float]or not math.isfinite(error)or not 0<=error<=1e-6 or r['archives']!=1:raise ValueError('geometry or archive gate failed')
    cases[r['id']][mode+'_'+stage+'_error']=error
  text=gzip.decompress((proof/('pick-command-'+mode+'.log.gz')).read_bytes()).decode();rows=[json.loads(s)for s in re.findall(r'^LINE_PICK_CONTEXT_SOURCE (.+)$',text,re.M)]
  if len(rows)!=14 or {r['id']for r in rows}!=set(matched):raise ValueError('incomplete or duplicate projected source query')
  for r in rows:
   error=r['maximum_projection_point_and_native_contact_error']
   if type(error)not in[int,float]or not math.isfinite(error)or not 0<=error<=1e-6 or not math.isfinite(r['world_metric_station_difference'])or abs(r['world_metric_station_difference']-world_errors[r['id']])>1e-10:raise ValueError('source projection gate failed')
   cases[r['id']][mode+'_source_error']=error
 paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines();sha=lambda b:hashlib.sha256(b).hexdigest()
 if not {'crates/viboceros-geometry/src/curve_projected_point.rs','crates/viboceros-geometry/tests/curve_projected_point.rs','src/viewport/edge_point.rs','crates/viboceros-command/tests/line_pick_context.rs'}.issubset(paths):raise ValueError('implementation lacks shared projection query')
 return dict(scope='Shared bounded projected NURBS location, used by viewport edge/curve picks, with eight retained refinement intervals; native fixed Tangent source calibration in Top, Front and Perspective.',baseline_revision=git('rev-parse',BASELINE).decode().strip(),implementation_revision=revision,model_epsilon=1e-6,full_native_parity=False,
 native_observations=14,matching_native_workflows_per_build=14,native_physical_picks=picks,public_pick_context_queries_per_build=14,archive_round_trips_per_build=28,maximum_native_public_pick_to_fixed_contact_difference=native_error,
 maximum_coefficient_and_sample_difference=max(x for c in cases.values()for k,x in c.items()if k.endswith('_command_error')or k.endswith('_ui_error')),maximum_source_projection_difference=max(x for c in cases.values()for k,x in c.items()if k.endswith('_source_error')),cases=list(cases.values()),
 resolved_input_calibration=dict(previous_record='docs/line-tangent-rays-validation.json',previous_case='ray_fixed_elevated',replacement_case='pick_circle_perspective',native_output_and_input_ray_identical=True,world_metric_station_difference=world_errors['pick_circle_perspective'],explanation='World-space viewing-line distance and screen-space distance select different perspective stations. Public PickFrustumTest corroborates the native fixed contact. Independent projected queries resolve the earlier replay difference without using output geometry as an input.'),
 projection_work_bounds=dict(maximum_spans=4096,sampling_intervals_per_span=16,retained_intervals=8,allocated_interval_slots=9,refinement_iterations=48),captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],
 input_calibration='Independently prescribed curves are minimized through the public pick-frustum transform targeting [0,0]. Public picked points and constructed output coordinates are expected values only; they do not seed or fit the Rust query. Fixed endpoints use the pre-click viewing line.',
 execution='Native captures used private Xvfb sessions; egui checks are windowless.',limits=['The bounded sampler/refiner is a numerical locator, not a certified global minimum. Eye-plane crossings, remote coordinates, singular projections and exhaustive ambiguous picks remain unqualified.',
 'Fourteen native contexts qualify the shared projection query and fixed-line replays. Windowless viewport events cover the application integration; arbitrary native camera replication and general pick-frustum admission/depth ranking remain unqualified.',
 'Coefficients, weights, knots, domains, attributes and 33 locally evaluated output samples are compared. Independent native PointAt output sampling, continuous distance and comparative speed remain unqualified.',
 'Earlier native tangent-search failures remain unresolved. The old mouse record and its historical world-metric replay diagnostic remain immutable.'],
 implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args();(ROOT/'docs/line-pick-context-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
