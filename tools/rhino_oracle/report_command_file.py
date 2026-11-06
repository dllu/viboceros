"""Rebuild bounded command-file evidence, retaining the layer Redo discrepancy."""
import argparse,gzip,hashlib,json,math,re,subprocess
from pathlib import Path
from .check_command_file import ROOT,PROOF,commands
HERE=Path(__file__).parent
BASELINE='81d3287b506b3f83208e5ecb7ad9e932c30ff04c'
FAMILIES={'command_file':12,'command_file_holdouts':6,'command_file_extras':6,'command_file_polyline':4}
FAILURES={'bang_middle','bang_incomplete','incomplete_line','unknown_command'}
GAPS={'layer_options'}
CORRECTIONS={
    'command':{'curve_cut::tests::native_polyline_cuts_retain_station_parameters_and_undo':'cuts',
        'curve_two_views::tests::reconstructs_profiles_with_fresh_unselected_output_and_one_undo':'two-views',
        'tests::curves::closure::closes_polylines_without_changing_identity_attributes_or_groups':'closure'},
    'app':{'app::tests::point_input::invalid_polyline_close_preserves_points_plane_reference_and_redo':'polyline-portable'}}

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '') is None:raise ValueError('full implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    sha=lambda b:hashlib.sha256(b).hexdigest()
    checks=json.loads((proof/'checks.json').read_text())
    if [(c['name'],c['command']) for c in checks]!=list(commands()) or any(c['status'] for c in checks):raise ValueError('incomplete or failed checks')
    receipts=json.loads((proof/'captures.json').read_text())
    if len(receipts)!=4 or {r['family'] for r in receipts}!=set(FAMILIES):raise ValueError('native receipts differ')
    evidence={p for p in proof.iterdir() if p.is_file()};native={};captures=[]
    sidecar=HERE/'fixtures/command_file_sources.json';evidence.add(sidecar)
    from .command_file_cases import CASES,request
    if json.loads(sidecar.read_bytes())!=CASES:raise ValueError('prescribed file sources changed')
    for family,count in FAMILIES.items():
        path=HERE/('observations/'+family+'.json.gz');raw=gzip.decompress(path.read_bytes());data=json.loads(raw)
        fixture=HERE/('fixtures/'+family+'.json');query=json.loads(fixture.read_bytes())
        if len(data['results'])!=count or [r['id'] for r in data['results']]!=[o['id'] for o in query['operations']] or query!=request([r['id'] for r in data['results']]):raise ValueError('native coverage differs')
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
            k,v=row['id'],row['value']
            if k in native or not v['returned_idle'] or v['success']!=(k not in FAILURES):raise ValueError('unexpected native completion')
            if 'Unknown command' in v['history'] and k not in {'bang_middle','unknown_command'}:raise ValueError('unexpected native recovery')
            for trace in v['input_trace']:
                if trace['errors'] or trace['completed']!=trace['expected']:raise ValueError('incomplete native input')
            edited=v['before']!=v['after']
            if len(v['undo_steps'])!=int(edited) or len(v['redo_steps'])!=int(edited):raise ValueError('native history scope changed')
            if edited and (v['undo_steps'][-1]!=v['before'] or v['redo_steps'][-1]!=v['after']):raise ValueError('native scene history differs')
            native[k]=v
    if set(native)!=set(CASES) or len(native)!=28 or sum(v['before']!=v['after'] for v in native.values())!=26:raise ValueError('native scope changed')
    cases={k:dict(id=k,success=v['success']) for k,v in native.items()}
    for mode in ['portable','native']:
        text=gzip.decompress((proof/('ui-'+mode+'.log.gz')).read_bytes()).decode()
        rows=[json.loads(s) for s in re.findall(r'^COMMAND_FILE_UI (.+)$',text,re.M)]
        if len(rows)!=28 or {r['id'] for r in rows}!=set(native):raise ValueError('incomplete or duplicate UI replay')
        if {r['id'] for r in rows if r['layer_history_matches_native'] is not True}!=GAPS:raise ValueError('layer Redo gap scope changed')
        for row in rows:
            k=row['id'];error=row['maximum_coefficient_and_sample_error'];edited=native[k]['before']!=native[k]['after']
            if type(error) not in [int,float] or not math.isfinite(error) or not 0<=error<=1e-6:raise ValueError('geometry gate failed: '+k)
            if row['archives']!=int(edited) or row['undo_steps']!=int(edited):raise ValueError('history or archive coverage differs')
            cases[k][mode+'_error']=error
    regressions=[]
    for suite,corrections in CORRECTIONS.items():
        text=gzip.decompress((proof/('initial-broad-'+suite+'.log.gz')).read_bytes()).decode()
        failed=set(re.findall(r'^test (\S+) \.\.\. FAILED$',text,re.M))
        summary=re.findall(r'^test result: FAILED\. (\d+) passed; (\d+) failed; (\d+) ignored;',text,re.M)
        if failed!=set(corrections) or len(summary)!=1 or int(summary[0][1])!=len(failed):raise ValueError('broad regression diagnostics changed')
        for test,check in corrections.items():
            log=gzip.decompress((proof/(check+'.log.gz')).read_bytes()).decode()
            if 'test '+test+' ... ok' not in log:raise ValueError('missing corrected regression: '+test)
        regressions.append(dict(suite=suite,initial_passed=int(summary[0][0]),initial_failed=int(summary[0][1]),initial_ignored=int(summary[0][2]),
            corrected_tests=corrections,reason='SDK-index test inputs now use the SDK constructor; command expectations retain physical parameters. Overflowing physical domains remain rejected without consuming Redo. Focused rechecks pass.'))
    sources=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    if not {'src/app/command_file.rs','crates/viboceros-command/src/command_macro.rs','crates/viboceros-document/src/history_batch.rs'}.issubset(sources):raise ValueError('revision does not contain the command-file implementation')
    return dict(scope='ReadCommandFile through application getters, nested files, pauses, atomic completed-command history and physical command-polyline parameters.',
        baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,
        full_native_parity=False,continuous_native_distance_qualified=False,performance_qualified=False,
        native_observations=28,qualified_geometry_workflows_per_build=28,successful_files_per_build=24,
        partial_failure_files_per_build=2,no_edit_failure_files_per_build=2,archive_round_trips_per_build=26,
        maximum_coefficient_and_sample_difference=max(v for c in cases.values() for k,v in c.items() if k.endswith('_error')),
        layer_redo_gaps=sorted(GAPS),captures=captures,cases=list(cases.values()),regression_diagnostics=regressions,
        execution='Rhino ran in private Xvfb sessions. Application and egui tests created no desktop windows.',
        limits=['The layer_options native Redo locks the new layer and leaves the previous layer current. Local Redo restores the original unlocked/current final state. This metadata discrepancy is explicit; geometry and construction/Undo layers match.',
            'Curve control points, weights, knots and 33 samples are compared. Samples evaluate captured native definitions locally, rather than independent native PointAt evaluations. Surface checks use control nets, knots and 9x9 grids, face sense, topology counts, vertices and trim types; complete trim ordering and continuous distance are unqualified.',
            'Only existing application getters consume staged tokens. The standalone kernel registry cannot execute files; commands requiring unimplemented getters and advanced repetition, transparent-command and redraw macro syntax remain unsupported. Pause coverage is point input; MultiPause covers an entire point getter.',
            'Files are limited to 16 MiB, one million consumed inputs and sixteen nesting frames. Processing yields between inputs, not inside a long modeling operation. UTF-8 and BOM-marked UTF-16 are supported.',
            'Whole-line kernel Layer New remains an existing create-and-set-current extension. Staged Layer New follows captured native create-only behavior. Comparative speed and arbitrary mouse trajectories are unqualified.'],
        checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in sources},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(evidence)})

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);args=p.parse_args()
    (ROOT/'docs/command-file-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
