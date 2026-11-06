"""Rebuild incremental grid qualification without admitting the timeout control."""
import argparse,gzip,hashlib,json,math,re,subprocess
from pathlib import Path
from .check_point_grid_input import ROOT,PROOF,commands
HERE=Path(__file__).parent
BASELINE='f1bd2fcbfd29abad588ba80c3ea23b4194038f83'
FAMILIES={'point_grid_input':12,'point_grid_input_holdouts':10,'point_grid_input_prompts':3}
DIAGNOSTIC='control_separate_degree'

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '') is None:raise ValueError('full implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    sha=lambda b:hashlib.sha256(b).hexdigest()
    checks=json.loads((proof/'checks.json').read_text())
    if [(c['name'],c['command']) for c in checks]!=list(commands()) or any(c['status'] for c in checks):raise ValueError('incomplete or failed checks')
    receipts=json.loads((proof/'captures.json').read_text())
    if len(receipts)!=3 or {r['family'] for r in receipts}!=set(FAMILIES):raise ValueError('native receipts differ')
    evidence={p for p in proof.iterdir() if p.is_file()};native={};captures=[];diagnostic=None
    for family,count in FAMILIES.items():
        path=HERE/('observations/'+family+'.json.gz');raw=gzip.decompress(path.read_bytes());data=json.loads(raw)
        fixture=HERE/('fixtures/'+family+'.json');query=json.loads(fixture.read_bytes())
        if len(data['results'])!=count or [r['id'] for r in data['results']]!=[o['id'] for o in query['operations']]:raise ValueError('native coverage differs')
        if data['provenance']['request_sha256']!=sha(fixture.read_bytes()):raise ValueError('native recipe changed')
        evidence.update([path,fixture])
        for digest in data['provenance']['sources'].values():
            archive=HERE/('observations/source_snapshots/'+digest+'.gz')
            if sha(gzip.decompress(archive.read_bytes()))!=digest:raise ValueError('producer archive changed')
            evidence.add(archive)
        receipt=next(r for r in receipts if r['family']==family)
        log=gzip.decompress((proof/receipt['log']).read_bytes()).decode()
        if receipt['exit_status']!=0 or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family] or log.splitlines()!=['Capturing '+str(count)+' cases: '+family,'Recorded '+family]:raise ValueError('native capture incomplete')
        captures.append(dict(family=family,observations=count,raw_sha256=sha(raw),engine_version=data['engine_version']))
        for row in data['results']:
            k,v=row['id'],row['value'];t=v['input_trace'][0]
            if not v['returned_idle'] or 'Unknown command' in v['history'] or v['after']!=v['redo']:raise ValueError('unexpected recovered native input or history')
            if k==DIAGNOSTIC:
                if diagnostic is not None or t['completed']!=1 or not t['errors'] or v['success'] or v['before']!=v['after']:raise ValueError('timeout diagnostic changed')
                diagnostic=dict(id=k,errors=t['errors'],qualified=False);continue
            if k in native or t['errors'] or t['completed']!=t['expected']:raise ValueError('incomplete or duplicate native input')
            native[k]=v
    if len(native)!=24 or sum(v['success'] for v in native.values())!=18 or diagnostic is None:raise ValueError('native scope changed')
    cases={k:dict(id=k,success=v['success']) for k,v in native.items()}
    for mode in ['portable','native']:
        text=gzip.decompress((proof/('ui-'+mode+'.log.gz')).read_bytes()).decode()
        rows=[json.loads(s) for s in re.findall(r'^POINT_GRID_UI (.+)$',text,re.M)]
        if len(rows)!=24 or {r['id'] for r in rows}!=set(native):raise ValueError('incomplete or duplicate UI replay')
        for row in rows:
            k=row['id'];error=row['maximum_coefficient_and_sample_error']
            if type(error) not in [int,float] or not math.isfinite(error) or not 0<=error<=1e-6:raise ValueError('geometry gate failed: '+k)
            if row['archives']!=int(native[k]['success']):raise ValueError('archive coverage differs')
            cases[k][mode+'_error']=error
    sources=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    return dict(scope='Incremental SrfPtGrid and SrfControlPtGrid count/degree/point getters, remembered preferences, local Undo and atomic creation.',
        baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,
        full_native_parity=False,continuous_native_distance_qualified=False,performance_qualified=False,
        native_observations=25,qualified_workflows_per_build=24,constructions_per_build=18,controls_per_build=6,archive_round_trips_per_build=18,
        maximum_coefficient_and_sample_difference=max(v for c in cases.values() for k,v in c.items() if k.endswith('_error')),
        parameter_grid=[9,9],captures=captures,cases=list(cases.values()),diagnostics=[diagnostic],
        execution='Rhino ran in private Xvfb sessions. Application and egui tests created no desktop windows.',
        limits=['One frozen probe missed the Row degree prompt and timed out. It is retained as an unqualified diagnostic; a corrected fresh capture covers that workflow.',
            'Every face control net, knot vector, face sense and finite parameter grid is checked. Topology counts, vertices and trim types are checked, but complete trim-index ordering and curve-domain parity are not qualified here.',
            'Native cancellation Undo/Redo fields are passive snapshots; local cancellation, invalid geometry and count-Undo tests separately retain pending Redo.',
            'A wire preview connects entered grid points. The fitted surface is constructed only after the required final point.',
            'Existing solver conditioning, 256-station and 4096-face limits remain. Continuous distance, arbitrary mouse trajectories, application-restart persistence and comparative speed are unqualified.'],
        checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in sources},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(evidence)})

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);args=p.parse_args()
    (ROOT/'docs/point-grid-input-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
