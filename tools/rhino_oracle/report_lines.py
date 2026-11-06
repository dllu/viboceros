"""Rebuild Lines qualification with explicit native-invalid and driver diagnostics."""
import argparse,gzip,hashlib,json,math,re,subprocess
from pathlib import Path
from .check_lines import ROOT,PROOF,commands
HERE=Path(__file__).parent
BASELINE='6269660124ff2e7b754dcccbc17b562667341411'
FAMILIES={'lines_command':10,'lines_holdouts':10,'lines_precision':11,'lines_finish':11,'lines_boundaries':4,'lines_macros':4}
GENERATORS={k:'generate_'+k for k in FAMILIES};GENERATORS['lines_command']='generate_lines'
INCOMPLETE={'explicit_return':('explicit_return_finish',4),'enter_anchor':('enter_anchor_finish',2),'enter_empty':('enter_empty_finish',1)}
INVALID={'translated_ulp','translated_small'}
KERNEL={'open_two','open_four','closed_three','closed_reversed','nonplanar','translated','spatial_plane','short_segments','coarse_tolerance'}

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '') is None:raise ValueError('full implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    sha=lambda b:hashlib.sha256(b).hexdigest();checks=json.loads((proof/'checks.json').read_text())
    if [(c['name'],c['command']) for c in checks]!=list(commands()) or any(c['status'] for c in checks):raise ValueError('incomplete or failed checks')
    receipts=json.loads((proof/'captures.json').read_text())
    if len(receipts)!=6 or {r['family'] for r in receipts}!=set(FAMILIES):raise ValueError('native receipts differ')
    from .lines_cases import CASES,request
    sidecar=HERE/'fixtures/lines_sources.json'
    if json.loads(sidecar.read_bytes())!=CASES:raise ValueError('prescribed sources changed')
    evidence={p for p in proof.iterdir() if p.is_file()}|{sidecar};native={};captures=[];diagnostics=[]
    for family,count in FAMILIES.items():
        path=HERE/('observations/'+family+'.json.gz');raw=gzip.decompress(path.read_bytes());data=json.loads(raw)
        fixture=HERE/('fixtures/'+family+'.json');query=json.loads(fixture.read_bytes());names=[r['id'] for r in data['results']]
        if len(names)!=count or query!=request(names):raise ValueError('native coverage differs')
        if data['provenance']['request_sha256']!=sha(fixture.read_bytes()):raise ValueError('native recipe changed')
        evidence.update([path,fixture])
        for digest in data['provenance']['sources'].values():
            archive=HERE/('observations/source_snapshots/'+digest+'.gz')
            if sha(gzip.decompress(archive.read_bytes()))!=digest:raise ValueError('producer archive changed')
            evidence.add(archive)
        receipt=next(r for r in receipts if r['family']==family);log=gzip.decompress((proof/receipt['log']).read_bytes()).decode()
        if receipt['exit_status']!=0 or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.'+GENERATORS[family]] or log.splitlines()!=['Capturing '+str(count)+' cases: '+family,'Recorded '+family]:raise ValueError('native capture incomplete')
        captures.append(dict(family=family,observations=count,raw_sha256=sha(raw),engine_version=data['engine_version']))
        for row in data['results']:
            k,v=row['id'],row['value'];t=v['input_trace'][0] if v['input_trace'] else None
            if k in native or not v['returned_idle'] or v['created']!=v['after'] or v['after']!=v['redo'] or v['before']!=v['undo']:raise ValueError('native completion or history differs')
            if 'Unknown command' in v['history'] and k not in {'close_two_cancel','close_anchor_cancel'}:raise ValueError('unexpected native recovery')
            if CASES[k].get('macro'):
                if t is not None:raise ValueError('unexpected macro input driver')
            elif k in INCOMPLETE:
                if t['completed']!=INCOMPLETE[k][1] or t['expected']!=t['completed']+1 or t['errors']!=['incomplete owned Lines input']:raise ValueError('driver diagnostic changed')
                diagnostics.append(dict(id=k,kind='early-completion input driver',corrected_by=INCOMPLETE[k][0],qualified=False))
            elif t['errors'] or t['completed']!=t['expected']:raise ValueError('incomplete native input')
            valid=all(o['native_valid'] for o in v['after'])
            if valid!=(k not in INVALID):raise ValueError('native-invalid scope changed')
            if not valid:diagnostics.append(dict(id=k,kind='native SDK marks a created line invalid',coefficients_compared=True,qualified=False))
            native[k]=v
    if set(native)!=set(CASES) or len(native)!=50:raise ValueError('native scope changed')
    for k,(corrected,_) in INCOMPLETE.items():
        if native[k]['after']!=native[corrected]['after'] or native[k]['success']!=native[corrected]['success']:raise ValueError('corrected native workflow changed')
    complete={k:v for k,v in native.items() if k not in INCOMPLETE}
    if sum(v['success'] for k,v in complete.items() if k not in INVALID)!=33:raise ValueError('native construction scope changed')
    cases={k:dict(id=k,success=v['success'],native_valid=k not in INVALID,qualified=k not in INVALID) for k,v in complete.items()}
    for mode in ['portable','native']:
        for stage,expected in [('ui',set(complete)),('command',KERNEL)]:
            text=gzip.decompress((proof/('lines-'+stage+'-'+mode+'.log.gz')).read_bytes()).decode()
            rows=[json.loads(s) for s in re.findall(r'^LINES_'+stage.upper()+r' (.+)$',text,re.M)]
            if len(rows)!=len(expected) or {r['id'] for r in rows}!=expected:raise ValueError('incomplete or duplicate Lines replay')
            for row in rows:
                k=row['id'];error=row['maximum_coefficient_and_sample_error']
                if type(error) not in [int,float] or not math.isfinite(error) or not 0<=error<=1e-6:raise ValueError('geometry gate failed: '+k)
                if stage=='ui' and row['native_valid'] is not (k not in INVALID):raise ValueError('native-validity replay scope changed')
                if row['archives']!=int(complete[k]['success'] and k not in INVALID):raise ValueError('archive coverage differs')
                cases[k][mode+'_'+stage+'_error']=error
    sources=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    if not {'crates/viboceros-command/src/lines.rs','src/app/curve_prompt.rs','src/app/drafting_input.rs'}.issubset(sources):raise ValueError('revision lacks the Lines implementation')
    return dict(scope='Adjoining independent Lines objects, shared point admission, Close, local Undo, exact return completion and atomic document history.',
        baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,full_native_parity=False,continuous_native_distance_qualified=False,performance_qualified=False,
        native_observations=50,complete_native_inputs_per_build=47,qualified_valid_native_workflows_per_build=45,constructions_per_build=33,cancel_controls_per_build=12,
        kernel_constructions_per_build=9,archive_round_trips_per_build=42,maximum_coefficient_and_sample_difference=max(v for c in cases.values() for k,v in c.items() if k.endswith('_error')),
        captures=captures,cases=list(cases.values()),diagnostics=diagnostics,checks=[dict(name=c['name'],status=c['status']) for c in checks],
        execution='Native Rhino ran in private Xvfb sessions. Rust application tests opened no desktop windows.',
        limits=['Three initial probes supplied a trailing token after Lines had already completed. They remain unqualified driver diagnostics; corrected fresh captures consume every prescribed token.',
            'Two extreme-coordinate native outputs contain SDK-invalid lines. Their coefficients, metadata and history are compared at the same model epsilon, but native-valid geometry and 3DM round trips are unqualified for those observations.',
            'Direct Lines snapshots preserve existing selection when recalling the created set; command-file snapshots replace it. Replays explicitly set DeselectOthersBeforeSelect=No/Yes to match those captured policies; default preference parity is unqualified.',
            'Curve controls, weights, knots and 33 normalized-parameter samples are checked. Samples evaluate captured definitions locally; independent native PointAt sampling and continuous Hausdorff distance are unqualified.',
            'Drafts remain outside the document until completion. Existing typed/picked coordinate and constraint routes are reused; native captures use typed points. Arbitrary mouse trajectories, comparative speed and more than 100000 segments are unqualified.'],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in sources},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(evidence)})

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);args=p.parse_args()
    (ROOT/'docs/lines-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
