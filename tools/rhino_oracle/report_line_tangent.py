"""Reconstruct Line Tangent native input, geometry, history and archive evidence."""
import argparse,gzip,hashlib,json,math,re,subprocess
from pathlib import Path
from .check_line_tangent import ROOT,PROOF,commands
HERE=Path(__file__).parent
BASELINE='e7f8285ba3b79d2c161b46b217d8619d9eba1bae'
FAMILIES={'line_tangent':22,'line_tangent_holdouts':20,'line_tangent_final':20,'line_tangent_completion':11}
DIAGNOSTICS={
 **{k:'The prescribed endpoint has no tangent in the source domain; the driver timed out without a correcting point.' for k in ['line_tangent','circle_spatial_end','circle_inside','arc_tangent','quadratic_tangent','spatial_tangent','tangent_both','numeric_then_point','arc_right']},
 **{k:'A length lock still needed a direction point; the prescribed input timed out.' for k in ['numeric_only','fixed_numeric','oncurve_negative','fixed_negative']},
 'planar_circle':'Planar accepted the first endpoint, so the extra point was never sent.',
 'oncurve_both_length':'BothSides with a fixed tangent accepted the number immediately, so the extra point was never sent.',
}
KNOWN_DIFFERENCES={k:'This repeatable open-arc input makes Rhino use its end tangent and project the destination; Viboceros finds the interior tangent reaching the typed point.' for k in ['arc_planar_tangent','arc_middle_repeat_a','arc_middle_repeat_b']}
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
 investigation_log=gzip.decompress((proof/'arc-investigation.log.gz').read_bytes()).decode()
 investigation_rows=[json.loads(s)for s in re.findall(r'^LINE_TANGENT_UI (.+)$',investigation_log,re.M)]
 investigation=next((r for r in investigation_rows if r['id']=='arc_planar_tangent'),None)
 if investigation is None or investigation['maximum_coefficient_and_sample_error']!=1.0:raise ValueError('known arc difference evidence missing')
 from .line_tangent_cases import CASES,request
 sidecar=HERE/'fixtures/line_tangent_sources.json'
 if json.loads(sidecar.read_bytes())!=CASES:raise ValueError('prescribed sources changed')
 evidence={p for p in proof.iterdir()if p.is_file()}|{sidecar};native={};captures=[];clicks=motions=0;diagnostics=[];known=[];seen=set()
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
   if k in seen or not v['returned_idle']or [dict(o,selected=0)for o in v['created']]!=[dict(o,selected=0)for o in v['after']]or v['after']!=v['redo']:raise ValueError('native completion or history differs')
   seen.add(k)
   if any(not o['native_valid']for o in v['after']):raise ValueError('native validity or selection changed')
   if [dict(o,selected=0)for o in v['before']]!=[dict(o,selected=0)for o in v['undo']]:raise ValueError('native undo changed source geometry')
   if k in DIAGNOSTICS:
    diagnostics.append(dict(id=k,reason=DIAGNOSTICS[k],completed=t['completed'],expected=t['expected'],errors=t['errors']))
    continue
   if t['errors']or t['completed']!=t['expected']or t['expected']!=len(CASES[k]['actions'])or 'Unknown command'in v['history']:raise ValueError('incomplete native input')
   actions=CASES[k]['actions'];expected_picks={i for i,a in enumerate(actions)if isinstance(a,dict)and'hover'not in a};actual=t.get('clicks',[])
   if len(actual)!=len(expected_picks)or {c['action']for c in actual}!=expected_picks or any(not c['motion_acknowledged']or not c['click_acknowledged']for c in actual):raise ValueError('incomplete native pointer input')
   for c in actual:
    action=actions[c['action']]
    if c['aim']!=action.get('pick',action.get('click')):raise ValueError('native pointer target changed')
    if 'curve'in action and (c.get('sight_point',c.get('projected_point'))is None or c['curve']!=action['curve']):raise ValueError('curve pick calibration missing')
   expected_hovers={i for i,a in enumerate(actions)if isinstance(a,dict)and'hover'in a};hovers=t.get('motion',[])
   if {h['action']for h in hovers}!=expected_hovers or any(not h['acknowledged'] or h['aim']!=actions[h['action']]['hover']for h in hovers):raise ValueError('incomplete native hover input')
   endpoints={i for i,a in enumerate(actions)if isinstance(a,dict)and'click'in a}
   endpoint_clicks=[c for c in actual if c['action']in endpoints]
   if len(endpoint_clicks)!=len(endpoints):raise ValueError('incomplete native endpoint clicks')
   if t.get('snap_settings',{}).get('planar',False)!=CASES[k].get('planar',False):raise ValueError('native Planar setting was not applied')
   clears=any(a in ['2Curves','PointOnCurve']for a in actions if isinstance(a,str))
   selected=[o['selected']for o in v['after']]
   if selected!=([0]*len(selected)if clears else [o['selected']for o in v['before']]+([0]if v['success']else[])):raise ValueError('native selection differs')
   if len(v['after'])!=len(v['before'])+int(v['success']):raise ValueError('unexpected native output count')
   if k in KNOWN_DIFFERENCES:
    known.append(dict(id=k,reason=KNOWN_DIFFERENCES[k],native_curve=v['after'][-1]['curve']));continue
   clicks+=len(actual);motions+=len(endpoint_clicks)
   if len(v['after'])!=len(v['before'])+int(v['success']):raise ValueError('unexpected native output count')
   native[k]=v
 if set(native)!=set(CASES)-set(DIAGNOSTICS)-set(KNOWN_DIFFERENCES)or {d['id']for d in diagnostics}!=set(DIAGNOSTICS)or len(native)!=55 or sum(v['success']for v in native.values())!=50 or clicks!=58 or motions!=1 or {r['id']for r in known}!=set(KNOWN_DIFFERENCES):raise ValueError('native scope changed')
 cases={k:dict(id=k,success=v['success'])for k,v in native.items()}
 for mode in ['portable','native']:
  for stage,expected in [('command',{k for k,v in native.items()if v['success']}),('ui',set(native))]:
   text=gzip.decompress((proof/('tangent-'+stage+'-'+mode+'.log.gz')).read_bytes()).decode()
   rows=[json.loads(s)for s in re.findall(r'^LINE_TANGENT_'+stage.upper()+r' (.+)$',text,re.M)]
   if len(rows)!=len(expected)or {r['id']for r in rows}!=expected:raise ValueError('incomplete or duplicate Line Tangent replay')
   for row in rows:
    k=row['id'];error=row['maximum_coefficient_and_sample_error']
    if type(error)not in [int,float]or not math.isfinite(error)or not 0<=error<=1e-6:raise ValueError('geometry gate failed: '+k)
    if row['archives']!=int(native[k]['success']):raise ValueError('archive coverage differs')
    cases[k][mode+'_'+stage+'_error']=error
 paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
 if not {'crates/viboceros-geometry/src/curve_tangent.rs','src/app/curve_line.rs','src/viewport/curve_line.rs'}.issubset(paths):raise ValueError('revision lacks Line Tangent input')
 return dict(scope='Line Tangent with sliding/fixed contacts, Point and PointOnCurve, signed lengths, BothSides, Planar drafting and bounded common tangents; three repeatable open-arc differences remain.',baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,
  full_native_parity=False,continuous_native_distance_qualified=False,performance_qualified=False,native_observations=sum(FAMILIES.values()),native_physical_curve_picks=clicks-motions,native_physical_endpoint_picks=motions,complete_native_inputs_per_build=len(native)+len(known),replayed_native_inputs_per_build=len(native),valid_constructions_per_build=sum(v['success']for v in native.values()),no_construction_controls_per_build=sum(not v['success']for v in native.values()),archive_round_trips_per_build=2*sum(v['success']for v in native.values()),
  maximum_coefficient_and_sample_difference=max(v for c in cases.values()for k,v in c.items()if k.endswith('_error')),captures=captures,diagnostics=diagnostics,known_geometry_differences=known,known_difference_investigation=dict(log='docs/line-tangent-proof/arc-investigation.log.gz',id='arc_planar_tangent',maximum_coefficient_and_sample_difference=1.0),cases=list(cases.values()),checks=[dict(name=c['name'],status=c['status'])for c in checks],
  execution='Native Rhino ran in private Xvfb sessions; Rust application tests created no desktop windows.',
  input_calibration='Physical picks use public SDK curve/viewport-ray measurements frozen before click acknowledgement. Early planar picks use public curve projection from the CPlane ray. Endpoint ray/constraint intersections are replayed independently of native output coefficients.',
  limits=['Three complete open-arc captures have a repeatable endpoint-tangent mismatch. They are retained separately and excluded from the passing replay counts. Full Line Tangent parity is not claimed.',
  'The catalog covers analytic lines/arcs/circles and polynomial spatial NURBS, both circle branches, common circle tangents, failed skew/nested pairs, rejected-point retries, signed lengths and Planar projection. Arbitrary singular curves and global completeness of tangent solutions are unqualified.',
  'Control points, weights, knots, domains and 33 normalized samples are compared. Samples evaluate captured definitions locally; independent native PointAt sampling, continuous distance and comparative performance are unqualified.',
  'Windowless mouse events exercise sliding/fixed contacts and preview/commit equality. Source lifetime, Brep edge bindings, units/calculator, macros, relative origins and transparent CPlane have Rust checks; their native variants, spatial mouse rays and native component-edge picking remain unqualified.',
  'Fifteen incomplete input recipes remain immutable driver diagnostics. Direct command replay omits the initial rejected point in complete retry workflows; UI replay exercises every prescribed input. Event-time highlighting, SelLast recall and arbitrary mouse trajectories remain unqualified.'],
  implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);args=p.parse_args()
 (ROOT/'docs/line-tangent-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
