"""Reconstruct bounded Tangent construction and public SDK evaluation evidence."""
import argparse,gzip,hashlib,json,math,re,subprocess
from .check_line_tangent_boundary import ROOT,PROOF,commands
from .tangent_boundary_evidence import HERE,FAMILIES,UI_FAMILIES,SDK_FAMILIES,DIAGNOSTICS,KNOWN,read,sdk_variants,failure_ranges
BASELINE='450350ed985684ad050056bf5f41248ae0ad8645'
def collect_native(proof):
 sha=lambda b:hashlib.sha256(b).hexdigest()
 from . import line_tangent_arcs as ui,line_tangent_arcs_sdk as sdk
 ui_sources=HERE/'fixtures/line_tangent_arcs_sources.json';sdk_sources=HERE/'fixtures/line_tangent_arcs_sdk_sources.json'
 if json.loads(ui_sources.read_bytes())!=ui.CASES or json.loads(sdk_sources.read_bytes())!=sdk.CASES:raise ValueError('prescribed sources changed')
 names={'line_tangent_arcs':ui.PILOT,'line_tangent_arcs_ends':list(ui.ENDS),'line_tangent_arcs_controls':list(ui.CONTROLS),'line_tangent_arcs_sdk':sdk.PILOT,'line_tangent_arcs_bounds':list(sdk.BOUNDS)}
 receipts=json.loads((proof/'captures.json').read_bytes())
 if len(receipts)!=len(FAMILIES)or {r['family']for r in receipts}!=set(FAMILIES):raise ValueError('capture receipts changed')
 evidence={p for p in proof.iterdir()if p.is_file()}|{ui_sources,sdk_sources,ROOT/'docs/line-tangent-validation.json'}
 matched={};known=[];diagnostics=[];profiles={};captures=[];seen=set();picks=0;total_queries=0
 for family,count in FAMILIES.items():
  path=HERE/('observations/'+family+'.json.gz');fixture=HERE/('fixtures/'+family+'.json');data=read(family)
  request=(ui.request if family in UI_FAMILIES else sdk.request)(names[family]);ids=[r['id']for r in data['results']]
  if ids!=names[family]or len(ids)!=count or json.loads(fixture.read_bytes())!=request:raise ValueError('native coverage changed')
  if data['provenance']['request_sha256']!=sha(fixture.read_bytes()):raise ValueError('native request changed')
  evidence.update([path,fixture])
  for digest in data['provenance']['sources'].values():
   archive=HERE/('observations/source_snapshots/'+digest+'.gz')
   if sha(gzip.decompress(archive.read_bytes()))!=digest:raise ValueError('producer archive changed')
   evidence.add(archive)
  receipt=next(r for r in receipts if r['family']==family);log=gzip.decompress((proof/receipt['log']).read_bytes()).decode()
  if receipt['exit_status']!=0 or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family]or log.splitlines()!=['Capturing '+str(count)+' cases: '+family,'Recorded '+family]:raise ValueError('capture incomplete')
  captures.append(dict(family=family,observations=count,raw_sha256=sha(gzip.decompress(path.read_bytes())),engine_version=data['engine_version']))
  for row in data['results']:
   k,v=row['id'],row['value']
   if k in seen:raise ValueError('duplicate native case')
   seen.add(k);variants=sdk_variants(v);queries=sum(len(c['queries'])for c in variants);total_queries+=queries
   if family in UI_FAMILIES:
    actions=ui.CASES[k]['actions'];t=v['input_trace'][0]
    if not v['success']or not v['returned_idle']or t['completed']!=len(actions)or t['expected']!=len(actions)or t['errors']or 'Unknown command'in v['history']:raise ValueError('incomplete native input')
    if v['after']!=v['redo']or len(v['after'])!=len(v['before'])+1 or [dict(o,selected=0)for o in v['before']]!=[dict(o,selected=0)for o in v['undo']]:raise ValueError('native history changed')
    if any(not o['native_valid']for o in v['after'])or [o['selected']for o in v['after']]!=[o['selected']for o in v['before']]+[0]:raise ValueError('native validity or selection changed')
    clicks=t.get('clicks',[]);expected={i for i,a in enumerate(actions)if isinstance(a,dict)and'curve'in a}
    if {c['action']for c in clicks}!=expected or len(clicks)!=len(expected)or any(not c['motion_acknowledged']or not c['click_acknowledged']or c.get('sight_point')is None or any(abs(a-b)>2*max(math.ulp(a),math.ulp(b))for a,b in zip(c['aim'],actions[c['action']]['pick']))for c in clicks):raise ValueError('curve pick receipt changed')
    hovers=t.get('motion',[]);expected_hovers={i for i,a in enumerate(actions)if isinstance(a,dict)and'hover'in a}
    if {h['action']for h in hovers}!=expected_hovers or any(not h['acknowledged']for h in hovers):raise ValueError('hover receipt changed')
    if t.get('snap_settings',{}).get('planar',False)!=ui.CASES[k].get('planar',False):raise ValueError('Planar setting changed')
    if 'prescribed_case'in v and v['prescribed_case']!=ui.CASES[k]:raise ValueError('native source recipe changed')
    if k in DIAGNOSTICS:
     diagnostics.append(dict(id=k,reason=DIAGNOSTICS[k]));continue
    if k in KNOWN:
     if variants[0]['queries'][0]['success'] or v['after'][-1]['curve']['control_points'][1]['point']==actions[-1]:raise ValueError('known local-search difference changed')
     known.append(dict(id=k,reason=KNOWN[k],native_curve=v['after'][-1]['curve']))
    else:matched[k]=v;picks+=len(clicks)
   elif v['case']!=sdk.CASES[k]:raise ValueError('native SDK recipe changed')
   failures=sum(not q['success']for c in variants for q in c['queries'])
   for c in variants:
    if [q['seed']for q in c['queries'][1:]]!=sorted(q['seed']for q in c['queries'][1:]):raise ValueError('SDK seed order changed')
    domain=c.get('search_domain',c['domain'])
    for q in c['queries']:
     if q['success'] and (not domain[0]<=q['parameter']<=domain[1]or any(not math.isfinite(x)for x in q['point']+q['tangent'])):raise ValueError('invalid public SDK contact')
   profiles[k]=dict(id=k,family=family,queries=queries,failures=failures,failure_intervals=[dict(representation=c['representation'],ranges=failure_ranges(c['queries'][1:]))for c in variants])
 if len(seen)!=50 or set(matched)!=set(ui.CASES)-set(KNOWN)-set(DIAGNOSTICS)or len(matched)!=24 or len(profiles)!=49 or total_queries!=34728 or picks!=24:raise ValueError('boundary coverage changed')
 return matched,profiles,known,diagnostics,captures,picks,total_queries,evidence
def build(proof,revision):
 if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full implementation revision required')
 git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
 git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
 sha=lambda b:hashlib.sha256(b).hexdigest()
 checks=json.loads((proof/'checks.json').read_bytes())
 if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete or failed checks')
 matched,profiles,known,diagnostics,captures,picks,total_queries,evidence=collect_native(proof)
 cases={k:dict(id=k)for k in matched}
 for mode in ['portable','native']:
  for stage in ['command','ui']:
   output=gzip.decompress((proof/('tangent-boundary-'+stage+'-'+mode+'.log.gz')).read_bytes()).decode()
   rows=[json.loads(s)for s in re.findall(r'^LINE_TANGENT_BOUNDARY_'+stage.upper()+r' (.+)$',output,re.M)]
   if len(rows)!=24 or {r['id']for r in rows}!=set(matched):raise ValueError('incomplete or duplicate boundary replay')
   for row in rows:
    error=row['maximum_coefficient_and_sample_error']
    if type(error)not in [int,float]or not math.isfinite(error)or not 0<=error<=1e-6:raise ValueError('geometry gate failed')
    if row['archives']!=1:raise ValueError('archive coverage differs')
    cases[row['id']][mode+'_'+stage+'_error']=error
  output=gzip.decompress((proof/('tangent-boundary-command-'+mode+'.log.gz')).read_bytes()).decode()
  rows=[json.loads(s)for s in re.findall(r'^LINE_TANGENT_CONTACTS (.+)$',output,re.M)]
  if len(rows)!=49 or {r['id']for r in rows}!=set(profiles):raise ValueError('incomplete public SDK evaluation')
  for row in rows:
   p=profiles[row['id']];error=row['maximum_point_tangent_and_residual_error']
   if row['family']!=p['family']or row['queries']!=p['queries']or row['failures']!=p['failures']or type(error)not in [int,float]or not math.isfinite(error)or not 0<=error<=1e-6:raise ValueError('public SDK evaluation gate failed')
   p[mode+'_point_tangent_and_residual_error']=error
 paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
 if not {'crates/viboceros-command/src/line/curve_input/construction.rs','crates/viboceros-command/tests/line_tangent_boundary.rs','src/app/tests/line_tangent_boundary.rs'}.issubset(paths):raise ValueError('implementation lacks bounded tangent behavior')
 return dict(scope='Line Tangent fallback to an open curve boundary, with signed lengths, BothSides, Planar projection, atomic zero-length rejection and mouse preview/relative input.',baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,full_native_parity=False,
  native_observations=50,complete_native_ui_inputs=33,matching_native_ui_inputs_per_build=24,archive_round_trips_per_build=48,native_physical_curve_picks_in_matching_inputs=picks,native_public_sdk_queries=total_queries,public_sdk_queries_covered_per_build=sum(p['queries']for p in profiles.values()),successful_public_sdk_points_evaluated_per_build=sum(p['queries']-p['failures']for p in profiles.values()),native_public_sdk_failures_in_covered_inputs=sum(p['failures']for p in profiles.values()),
  maximum_coefficient_and_sample_difference=max(x for c in cases.values()for k,x in c.items()if k.endswith('_error')),maximum_public_sdk_evaluation_difference=max(x for p in profiles.values()for k,x in p.items()if k.endswith('_error')),
  cases=list(cases.values()),sdk_profiles=list(profiles.values()),diagnostics=diagnostics,known_geometry_differences=known,captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],
  input_calibration='Physical source stations are measured from public viewport-ray queries before click acknowledgement. Prescribed trigonometric aims permit two ulps across Python runtimes; recorded geometry is not used to fit input stations.',
  execution='Native captures used private Xvfb sessions; application tests used windowless egui events.',
  limits=['Eight complete native workflows retain the seed-sensitive local tangent search mismatch. Their SDK query fails despite an interior tangent; they are excluded from passing construction counts. One integer-division source diagnostic is also excluded.',
  'Public SDK success points, tangents and tangent residuals are evaluated on independently prescribed analytic/NURBS curves. Native SDK failure parity is not claimed; dense seed sweeps are investigation evidence, not fitted production lookup tables.',
  'Control points, weights, knots, domains and 33 locally evaluated samples are compared for matching constructions. Independent native PointAt sampling of constructed outputs, continuous distance, arbitrary singular curves, all local-search branches and comparative speed remain unqualified.',
  'Native command and UI recipes cover the recorded typed inputs. Mouse preview/commit equality, relative origins, atomic rejection and reference storage have Rust checks; spatial native mouse rays and additional settings combinations remain unqualified.'],
  implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
 (ROOT/'docs/line-tangent-boundary-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
