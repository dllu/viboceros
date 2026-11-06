"""Reconstruct Line Normal native input, geometry, history and archive evidence."""
import argparse,gzip,hashlib,json,math,re,subprocess
from pathlib import Path
from .check_line_normal import ROOT,PROOF,commands
HERE=Path(__file__).parent
BASELINE='4415e935e200ee3e2e7c8ea5317c7658f985c567'
FAMILIES={'line_normal':16,'line_normal_holdouts':18,'line_normal_final':15,'line_normal_brep':4}
DIAGNOSTICS={
 'circle_length':'The numeric input completed before the prescribed extra endpoint.',
 'plane_length':'The numeric input completed before the prescribed extra endpoint.',
 'line_length':'A straight-curve length lock awaited a direction point and timed out.',
 'surface_reversed':'The acknowledged wireframe interior click did not accept the reference.',
 'trimmed_no':'The wireframe interior click did not accept the reference; input ended early.',
 'trimmed_yes':'The wireframe interior click did not accept the reference; input ended early.',
 'trimmed_flipped':'The wireframe interior click did not accept the reference; input ended early.',
}
def observations():
 return {r['id']:r['value']for family in FAMILIES for r in json.loads(gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes()))['results']}
def build(proof,revision):
 if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full implementation revision required')
 git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
 git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
 sha=lambda b:hashlib.sha256(b).hexdigest();checks=json.loads((proof/'checks.json').read_text())
 if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete or failed checks')
 receipts=json.loads((proof/'captures.json').read_text())
 if len(receipts)!=len(FAMILIES)or {r['family']for r in receipts}!=set(FAMILIES):raise ValueError('native receipts differ')
 from .line_normal_cases import CASES,request
 sidecar=HERE/'fixtures/line_normal_sources.json'
 if json.loads(sidecar.read_bytes())!=CASES:raise ValueError('prescribed sources changed')
 evidence={p for p in proof.iterdir()if p.is_file()}|{sidecar};native={};captures=[];clicks=motions=0;diagnostics=[]
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
   if any(not o['native_valid']for o in v['after'])or any(o['selected']for o in v['after']):raise ValueError('native validity or selection changed')
   if [dict(o,selected=0)for o in v['before']]!=[dict(o,selected=0)for o in v['undo']]:raise ValueError('native undo changed source geometry')
   if k in DIAGNOSTICS:
    diagnostics.append(dict(id=k,reason=DIAGNOSTICS[k],completed=t['completed'],expected=t['expected'],errors=t['errors']))
    continue
   if t['errors']or t['completed']!=t['expected']or t['expected']!=len(CASES[k]['actions'])or 'Unknown command'in v['history']:raise ValueError('incomplete native input')
   actions=CASES[k]['actions'];expected_picks={i for i,a in enumerate(actions)if isinstance(a,dict)and'curve'in a};actual=t.get('clicks',[])
   if len(actual)!=len(expected_picks)or {c['action']for c in actual}!=expected_picks or any(not c['motion_acknowledged']or not c['click_acknowledged']or c['aim']!=actions[c['action']]['pick']for c in actual):raise ValueError('incomplete native reference picks')
   # A physical click receipt alone does not establish that GetObject accepted
   # the reference. The next public prompt must be its base-point getter.
   if any(not t['prompts'][i+1].startswith(('Start of line','Middle of line'))for i in expected_picks):raise ValueError('reference selection was not accepted')
   expected_motion={i for i,a in enumerate(actions)if isinstance(a,dict)and'hover'in a};motion=t['motion']
   if len(motion)!=len(expected_motion)or {m['action']for m in motion}!=expected_motion or any(not m['acknowledged']for m in motion):raise ValueError('incomplete native hover')
   clicks+=len(actual);motions+=len(motion)
   if len(v['after'])!=len(v['before'])+int(v['success']):raise ValueError('unexpected native output count')
   native[k]=v
 if set(native)!=set(CASES)-set(DIAGNOSTICS)or {d['id']for d in diagnostics}!=set(DIAGNOSTICS)or len(native)!=46 or sum(v['success']for v in native.values())!=39 or clicks!=44 or motions!=3:raise ValueError('native scope changed')
 cases={k:dict(id=k,success=v['success'])for k,v in native.items()}
 for mode in ['portable','native']:
  for stage,expected in [('command',{k for k,v in native.items()if v['success']}),('ui',set(native))]:
   text=gzip.decompress((proof/('normal-'+stage+'-'+mode+'.log.gz')).read_bytes()).decode()
   rows=[json.loads(s)for s in re.findall(r'^LINE_NORMAL_'+stage.upper()+r' (.+)$',text,re.M)]
   if len(rows)!=len(expected)or {r['id']for r in rows}!=expected:raise ValueError('incomplete or duplicate Line Normal replay')
   for row in rows:
    k=row['id'];error=row['maximum_coefficient_and_sample_error']
    if type(error)not in [int,float]or not math.isfinite(error)or not 0<=error<=1e-6:raise ValueError('geometry gate failed: '+k)
    if row['archives']!=int(native[k]['success']):raise ValueError('archive coverage differs')
    cases[k][mode+'_'+stage+'_error']=error
 paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
 if not {'crates/viboceros-command/src/line/normal.rs','src/app/line_normal.rs','src/app/viewport_input.rs'}.issubset(paths):raise ValueError('revision lacks Line Normal input')
 return dict(scope='Line Normal from immutable curve/surface/Brep references, curvature axes, zero-curvature normal planes, oriented faces, IgnoreTrims, signed numeric completion and BothSides.',baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,
  full_native_parity=False,continuous_native_distance_qualified=False,performance_qualified=False,native_observations=sum(FAMILIES.values()),native_physical_reference_picks=clicks,native_acknowledged_hovers=motions,complete_native_inputs_per_build=len(native),valid_constructions_per_build=sum(v['success']for v in native.values()),cancel_controls_per_build=sum(not v['success']for v in native.values()),archive_round_trips_per_build=2*sum(v['success']for v in native.values()),
  maximum_coefficient_and_sample_difference=max(v for c in cases.values()for k,v in c.items()if k.endswith('_error')),captures=captures,diagnostics=diagnostics,cases=list(cases.values()),checks=[dict(name=c['name'],status=c['status'])for c in checks],
  execution='Native Rhino ran in private Xvfb sessions; Rust application tests created no desktop windows.',
  limits=['The fixed catalog covers analytic and NURBS curves, straight/polyline normal planes, bilinear surfaces, rectangular trims, reversed faces and box polysurfaces. SubD references and arbitrary singular/inflected curves are unqualified.',
  'Line/curve coefficients, domains and 33 normalized samples are compared. Brep sources compare vertices, face coefficients/orientation, 81 surface samples, edge/loop counts and trim types; full edge/UV-trim coefficient parity is unqualified. Samples evaluate captured definitions locally; independent native PointAt sampling, continuous distance and comparative performance are unqualified.',
  'Windowless viewport tests cover curve/surface source and base picking and preview/commit agreement. Edge bindings, source lifetime, calculator/units, command files and transparent CPlane input have Rust checks; their native input variants are unqualified.',
  'Seven earlier driver diagnostics remain immutable and are excluded from complete-input and construction counts. Event-time source highlighting and arbitrary mouse trajectories are unqualified.',
  'Other Line reference modes Tangent and Perpendicular remain unsupported.'],
  implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);args=p.parse_args()
 (ROOT/'docs/line-normal-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
