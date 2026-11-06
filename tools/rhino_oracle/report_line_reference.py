"""Reconstruct native Line Extension geometry and reference-input qualification."""
import argparse,gzip,hashlib,json,math,re,subprocess
from pathlib import Path
from .check_line_reference import ROOT,PROOF,commands
HERE=Path(__file__).parent
BASELINE='0b8e5b0f458f06e73097443f6a0dc526915cab28'
FAMILIES={'line_reference':12,'line_reference_holdouts':14,'line_reference_completion':8,'line_reference_final':2}
def observations():
 return {r['id']:r['value']for family in FAMILIES for r in json.loads(gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes()))['results']}
def build(proof,revision):
 if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full implementation revision required')
 git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
 git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
 sha=lambda b:hashlib.sha256(b).hexdigest();checks=json.loads((proof/'checks.json').read_text())
 if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete or failed checks')
 receipts=json.loads((proof/'captures.json').read_text())
 if len(receipts)!=4 or {r['family']for r in receipts}!=set(FAMILIES):raise ValueError('native receipts differ')
 from .line_reference_cases import CASES,request
 sidecar=HERE/'fixtures/line_reference_sources.json'
 if json.loads(sidecar.read_bytes())!=CASES:raise ValueError('prescribed sources changed')
 evidence={p for p in proof.iterdir()if p.is_file()}|{sidecar};native={};captures=[];clicks=0
 for family,count in FAMILIES.items():
  path=HERE/('observations/'+family+'.json.gz');raw=gzip.decompress(path.read_bytes());data=json.loads(raw);fixture=HERE/('fixtures/'+family+'.json')
  names=[r['id']for r in data['results']]
  if len(names)!=count or json.loads(fixture.read_bytes())!=request(names):raise ValueError('native coverage differs')
  if data['provenance']['request_sha256']!=sha(fixture.read_bytes()):raise ValueError('native recipe changed')
  evidence.update([path,fixture])
  for digest in data['provenance']['sources'].values():
   archive=HERE/('observations/source_snapshots/'+digest+'.gz')
   if sha(gzip.decompress(archive.read_bytes()))!=digest:raise ValueError('producer archive changed')
   evidence.add(archive)
  receipt=next(r for r in receipts if r['family']==family);log=gzip.decompress((proof/receipt['log']).read_bytes()).decode()
  if receipt['exit_status']!=0 or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family]or log.splitlines()!=['Capturing '+str(count)+' cases: '+family,'Recorded '+family]:raise ValueError('native capture incomplete')
  captures.append(dict(family=family,observations=count,raw_sha256=sha(raw),engine_version=data['engine_version']))
  for row in data['results']:
   k,v=row['id'],row['value'];t=v['input_trace'][0]
   if k in native or not v['returned_idle']or [dict(o,selected=0)for o in v['created']]!=[dict(o,selected=0)for o in v['after']]or v['after']!=v['redo']:raise ValueError('native completion or history differs')
   if t['errors']or t['completed']!=t['expected']or t['expected']!=len(CASES[k]['actions'])or 'Unknown command'in v['history']:raise ValueError('incomplete native input')
   expected_picks={i for i,a in enumerate(CASES[k]['actions'])if isinstance(a,dict)}
   actual=t.get('clicks',[])
   if len(actual)!=len(expected_picks)or {c['action']for c in actual}!=expected_picks or any(not c['motion_acknowledged']or not c['click_acknowledged']for c in actual):raise ValueError('incomplete native curve picks')
   clicks+=len(actual)
   if any(not o['native_valid']for o in v['after']):raise ValueError('native validity changed')
   if v['success'] and len(v['after'])!=len(v['before'])+1:raise ValueError('unexpected native output count')
   if not v['success'] and len(v['after'])!=len(v['before']):raise ValueError('native cancellation edited geometry')
   if [dict(o,selected=0)for o in v['before']]!=[dict(o,selected=0)for o in v['undo']]:raise ValueError('native undo geometry changed')
   if any(o['selected']for o in v['after']):raise ValueError('native selection release changed')
   native[k]=v
 if set(native)!=set(CASES)or len(native)!=36 or sum(v['success']for v in native.values())!=30 or clicks!=34:raise ValueError('native scope changed')
 cases={k:dict(id=k,success=v['success'])for k,v in native.items()}
 for mode in ['portable','native']:
  for stage,expected in [('command',{k for k,v in native.items()if v['success']}),('ui',set(native))]:
   text=gzip.decompress((proof/('reference-'+stage+'-'+mode+'.log.gz')).read_bytes()).decode()
   rows=[json.loads(s)for s in re.findall(r'^LINE_REFERENCE_'+stage.upper()+r' (.+)$',text,re.M)]
   if len(rows)!=len(expected)or {r['id']for r in rows}!=expected:raise ValueError('incomplete or duplicate Line reference replay')
   for row in rows:
    k=row['id'];error=row['maximum_coefficient_and_sample_error']
    if type(error)not in [int,float]or not math.isfinite(error)or not 0<=error<=1e-6:raise ValueError('geometry gate failed: '+k)
    if row['archives']!=int(native[k]['success']):raise ValueError('archive coverage differs')
    cases[k][mode+'_'+stage+'_error']=error
 paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
 if not {'crates/viboceros-command/src/line/input.rs','src/app/line_reference.rs','src/app/viewport_input.rs'}.issubset(paths):raise ValueError('revision lacks Line reference input')
 return dict(scope='Line Extension with immutable curve references, endpoint selection by arc length, tangent projection, BothSides and length/increment constraints.',baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,
  full_native_parity=False,continuous_native_distance_qualified=False,performance_qualified=False,native_observations=36,native_physical_curve_picks=34,complete_native_inputs_per_build=36,valid_constructions_per_build=30,cancel_controls_per_build=6,kernel_constructions_per_build=30,archive_round_trips_per_build=60,
  maximum_coefficient_and_sample_difference=max(v for c in cases.values()for k,v in c.items()if k.endswith('_error')),captures=captures,cases=list(cases.values()),checks=[dict(name=c['name'],status=c['status'])for c in checks],
  execution='Native Rhino ran in private Xvfb sessions; Rust application tests created no desktop windows.',
  limits=['The native catalog covers ordinary line, arc, circle, polyline, polycurve and weighted NURBS guides, including closed seams, a folded curve and remote coordinates. Exact arc-length midpoint endpoint ties and arbitrary source types are unqualified.',
  'B-rep edge bindings are checked through read-only Rust resolution and parent-preserving history; native component-pick geometry is unqualified for this catalog.',
  'Viewport mouse events verify ordinary source picking, numeric Enter and preview/commit agreement. Command-file tokens, source replacement, transparent CPlane and calculator/unit routing are checked windowlessly; native whole-file and nonliteral numeric comparison for Extension are unqualified.',
  'Controls, weights, knots, domains and 33 normalized-parameter samples are compared. Samples evaluate captured definitions locally; independent native PointAt sampling, continuous distance and comparative performance are unqualified.',
  'Native EndCommand snapshots can retain a transient source highlight. Final selection is compared after command cleanup; event-time highlight state is unqualified.',
  'Other Line reference modes Normal, Tangent and Perpendicular remain unsupported.'],
  implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);args=p.parse_args()
 (ROOT/'docs/line-reference-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
