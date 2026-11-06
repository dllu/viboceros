"""Reconstruct native Tangent mouse modes and ruled-out search evidence."""
import argparse,gzip,hashlib,json,math,re,subprocess
from .check_line_tangent_rays import ROOT,PROOF,commands
from . import line_tangent_rays as ui,tangent_zero_sdk as zero
HERE=ROOT/'tools/rhino_oracle'
BASELINE='ffa15af647ffc80e45d6b6bca3c5081df62d46c8'
def read(family):return json.loads(gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes()))
def collect_native(proof):
 sha=lambda b:hashlib.sha256(b).hexdigest()
 source=HERE/'fixtures/line_tangent_rays_sources.json'
 if json.loads(source.read_bytes())!=dict(ui.CASES,**ui.CONFIRM):raise ValueError('prescribed sources changed')
 receipts=json.loads((proof/'captures.json').read_bytes())
 if [r['family']for r in receipts]!=['line_tangent_rays','line_tangent_mouse','tangent_zero_sdk']:raise ValueError('capture inventory changed')
 evidence={p for p in proof.iterdir()if p.is_file()}|{source,ROOT/'docs/line-tangent-boundary-validation.json'}
 captures=[];matched={};known={};diagnostics=[];picks=0;queries=0;trials=0;failures=0
 for family,module in [('line_tangent_rays',ui),('line_tangent_mouse',ui),('tangent_zero_sdk',zero)]:
  recipes=ui.CONFIRM if family=='line_tangent_mouse'else module.CASES
  request=ui.confirmation_request()if family=='line_tangent_mouse'else module.request()
  path=HERE/('observations/'+family+'.json.gz');fixture=HERE/('fixtures/'+family+'.json');data=read(family);ids=[r['id']for r in data['results']]
  if ids!=list(recipes)or json.loads(fixture.read_bytes())!=request:raise ValueError('native request coverage changed')
  if data['provenance']['request_sha256']!=sha(fixture.read_bytes()):raise ValueError('native request hash changed')
  evidence.update([path,fixture])
  for digest in data['provenance']['sources'].values():
   archive=HERE/('observations/source_snapshots/'+digest+'.gz')
   if sha(gzip.decompress(archive.read_bytes()))!=digest:raise ValueError('frozen producer changed')
   evidence.add(archive)
  receipt=next(r for r in receipts if r['family']==family);log=gzip.decompress((proof/receipt['log']).read_bytes()).decode().splitlines()
  if receipt['exit_status']!=(None if family=='line_tangent_rays'else 0) or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family]or log[-2:]!=['Capturing '+str(len(ids))+' cases: '+family,'Recorded '+family]:raise ValueError('capture incomplete')
  if family in ['line_tangent_rays','line_tangent_mouse']and len(log)!=2:raise ValueError('unexpected native capture diagnostics')
  if family=='tangent_zero_sdk'and len(log)!=4:raise ValueError('zero-search syntax warning receipt changed')
  captures.append(dict(family=family,observations=len(ids),raw_sha256=sha(gzip.decompress(path.read_bytes())),engine_version=data['engine_version']))
  for row in data['results']:
   k,v=row['id'],row['value']
   if family=='tangent_zero_sdk':
    if v['source']!=zero.CASES[k]or len(v['queries'])!=1001:raise ValueError('zero-search coverage changed')
    a,b=v['domain'];bad=[]
    for i,q in enumerate(v['queries']):
     queries+=1
     if abs(q['seed']-(a+(b-a)*i/1000.))>2*max(math.ulp(q['seed']),math.ulp(b)):raise ValueError('zero-search seed changed')
     if not q['native_success']:bad.append(i);failures+=1
     if {(t['function'],t['bracket'])for t in q['trials']}!={(f,b)for f in ['unit_cross','dot_normal','derivative_cross']for b in ['full','local']}or len(q['trials'])!=6:raise ValueError('zero-search callbacks changed')
     for t in q['trials']:
      trials+=1
      if t['parameter']is None or not a<=t['parameter']<=b or not math.isfinite(t['parameter'])or t['reason']not in ['round_newton','interval']:raise ValueError('zero-search convergence changed')
      parameter=t['parameter']
      if k=='zero_arc':x,y=5*math.cos(parameter/5),5*math.sin(parameter/5)
      else:
       b0,b1,b2=(1-parameter)**2,math.sqrt(2)*parameter*(1-parameter),parameter**2;weight=b0+b1+b2
       x,y=5*(b0+b1)/weight,5*(b1+b2)/weight
      if abs((-8*x+6*y)/5-5)>1e-6:raise ValueError('zero-search tangent residual changed')
    if bad!=list(range(502,520 if k=='zero_arc'else 514)):raise ValueError('native failure profile changed')
    continue
   t=v['input_trace'][0];actions=recipes[k]['actions'];clicks=t['clicks']
   if family=='line_tangent_rays'and k in ui.REJECTED:
    if t['errors']!=['owned Line input timeout']or v['success']or v['before']!=v['after']or t['completed']!=len(actions)or not v['returned_idle']:raise ValueError('pilot rejection diagnostic changed')
    diagnostics.append(dict(id=k,reason='The endpoint click left Line pending. The pilot hit its timer; the explicit-cancellation confirmation is required for passing workflow coverage.'));continue
   if v['success']!=(family=='line_tangent_rays')or not v['returned_idle']or t['errors']or t['completed']!=len(actions)or t['expected']!=len(actions)or v['prescribed_case']!=recipes[k]:raise ValueError('incomplete native input: '+k)
   if len(v['after'])!=len(v['before'])+(1 if v['success']else 0) or v['after']!=v['redo']or [dict(o,selected=0)for o in v['before']]!=[dict(o,selected=0)for o in v['undo']]:raise ValueError('native history changed: '+k)
   if any(not o['native_valid']for o in v['after'])or [o['selected']for o in v['after']]!=[o['selected']for o in v['before']]+([0]if v['success']else[]):raise ValueError('native object validity or selection changed')
   expected={i for i,a in enumerate(actions)if isinstance(a,dict)and ('curve'in a or'click'in a)}
   if {c['action']for c in clicks}!=expected or len(clicks)!=2:raise ValueError('physical picks missing')
   for c in clicks:
    aim=actions[c['action']].get('pick',actions[c['action']].get('click'))
    if not c['motion_acknowledged']or not c['click_acknowledged']or len(c['ray'])!=2 or any(not math.isfinite(x)for p in c['ray']for x in p)or any(abs(x-y)>2*max(math.ulp(x),math.ulp(y))for x,y in zip(c['aim'],aim)):raise ValueError('physical input receipt changed')
    if 'curve'in actions[c['action']]and c.get('sight_point')is None:raise ValueError('source station calibration missing')
   if t['snap_settings']['planar']!=recipes[k]['planar']:raise ValueError('Planar setting changed')
   if k=='ray_fixed_elevated':known[k]=v
   else:matched[k]=v;picks+=len(clicks)
 if len(matched)!=11 or len(known)!=1 or len(diagnostics)!=8 or picks!=22 or queries!=2002 or trials!=12012 or failures!=30:raise ValueError('mouse/search experiment coverage changed')
 return matched,known,diagnostics,captures,picks,queries,trials,failures,evidence

def build(proof,revision):
 if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full implementation revision required')
 git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
 git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
 checks=json.loads((proof/'checks.json').read_bytes())
 if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete or failed checks')
 matched,known,diagnostics,captures,picks,queries,trials,failures,evidence=collect_native(proof);cases={k:dict(id=k)for k in matched}
 for mode in ['portable','native']:
  for stage in ['command','ui']:
   output=gzip.decompress((proof/('tangent-rays-'+stage+'-'+mode+'.log.gz')).read_bytes()).decode()
   rows=[json.loads(s)for s in re.findall(r'^LINE_TANGENT_RAYS_'+stage.upper()+r' (.+)$',output,re.M)]
   if len(rows)!=len(matched)or {r['id']for r in rows}!=set(matched):raise ValueError('incomplete or duplicate viewing-ray replay')
   for row in rows:
    error=row['maximum_coefficient_and_sample_error']
    if type(error)not in [int,float]or not math.isfinite(error)or not 0<=error<=1e-6:raise ValueError('geometry gate failed')
    if row['archives']!=(1 if matched[row['id']]['success']else 0):raise ValueError('archive coverage differs')
    cases[row['id']][mode+'_'+stage+'_error']=error
 known_errors={}
 for mode in ['portable','native']:
  text=gzip.decompress((proof/('tangent-rays-command-'+mode+'.log.gz')).read_bytes()).decode()
  rows=[json.loads(s)for s in re.findall(r'^LINE_TANGENT_RAYS_KNOWN (.+)$',text,re.M)]
  if len(rows)!=1 or rows[0]['id']!='ray_fixed_elevated':raise ValueError('missing perspective pick diagnostic')
  error=rows[0]['maximum_coefficient_and_sample_error']
  if type(error)not in [int,float]or not math.isfinite(error)or not 1e-6<error<1e-3:raise ValueError('perspective pick diagnostic changed')
  known_errors[mode]=error
 paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines();sha=lambda b:hashlib.sha256(b).hexdigest()
 if not {'crates/viboceros-geometry/src/curve_contact.rs','crates/viboceros-command/tests/line_tangent_rays.rs','src/app/tests/line_tangent_rays.rs'}.issubset(paths):raise ValueError('implementation lacks mouse coverage and shared contact seeds')
 return dict(scope='Native Line Tangent mouse modes: three Planar/boundary constructions, one known fixed perspective pick difference and eight no-output sliding mouse workflows followed by explicit cancellation, preserving pending redo. Shared contact seed generation retains prior behavior.',baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,full_native_parity=False,
 native_observations=22,complete_native_ui_inputs=12,matching_native_ui_inputs_per_build=len(matched),native_constructions_per_build=3,native_no_output_workflows_per_build=8,rejected_endpoint_coordinates_per_build=7,edge_on_views_without_cplane_point_per_build=1,native_physical_picks_in_matching_inputs=picks,archive_round_trips_per_build=6,
 maximum_coefficient_and_sample_difference=max(x for c in cases.values()for k,x in c.items()if k.endswith('_error')),cases=list(cases.values()),diagnostics=diagnostics,known_geometry_differences=[dict(id='ray_fixed_elevated',reason='The public curve/viewing-line closest point and the native fixed picked station differ by about 1.12e-4 model units. Perspective source pick calibration remains unqualified; no native output coordinate is substituted as an input.',observed_errors=known_errors,native_curve=known['ray_fixed_elevated']['after'][-1]['curve'])],
 zero_search_experiment=dict(native_queries=queries,public_zero_finder_trials=trials,native_query_failures=failures,trial_failures=0,conclusion='Six public zero-finder callback/bracket combinations converge on every recorded seed. This experiment does not explain the native tangent search failures and is not used by the production solver.'),
 captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],
 input_calibration='Curves are independently prescribed. Public viewport queries measure source stations, CPlane projections and viewing lines before physical click acknowledgements. Fixed tangents use viewing-line projection; free sliding endpoints use the CPlane point, with Planar and boundary controls. Recorded output geometry is used only for comparisons.',
 command_input_path='Seven rejected endpoint coordinates are submitted to the curve getter; the Front viewing line is parallel to the CPlane and supplies no endpoint coordinate. CLI atomicity, UI cancellation, geometry, selection and redo are compared where applicable.',
 execution='Native observations use private Xvfb sessions. UI preview and click tests use windowless egui events.',
 limits=['Previously recorded native local tangent-search failures remain unresolved; no empirical failure intervals are embedded in production.',
 'Control points, weights, knots, domains and 33 locally evaluated output samples are compared. Independent native PointAt sampling of constructed outputs, continuous distance, singular curves, remote coordinates, comparative speed and exhaustive mouse branches remain unqualified.',
 'Eight pilot inputs timed out while the endpoint getter remained pending; they are diagnostic observations, excluded from passing counts. All eight require a separate clean explicit-cancellation capture. The pilot terminal tool status was unavailable after context truncation; its Recorded marker and immutable output prove capture validation completed, without inferring an exit status.',
 'Object snaps, point filters, Ortho, grid snap and tracking retain their existing paths. Additional native settings combinations and spatial fallback branches remain unqualified.'],
 implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
 (ROOT/'docs/line-tangent-rays-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
