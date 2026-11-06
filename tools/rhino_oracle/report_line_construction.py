"""Reconstruct bounded Line qualification with explicit driver/validity limits."""
import argparse,gzip,hashlib,json,math,re,subprocess
from pathlib import Path
from .check_line_construction import ROOT,PROOF,commands
HERE=Path(__file__).parent
BASELINE='10da289311513a7f56918653aee20e096700e208'
FAMILIES={'line_construction':24,'line_construction_holdouts':25,'line_construction_precision':10,'line_construction_completion':9}
TIMEOUT=['owned Line input timeout'];EARLY=['incomplete owned Line input']
# Initial malformed/unfinished recipes are immutable and never qualify as passes.
INCOMPLETE={
 'both_initial':(1,3,TIMEOUT+EARLY,'both_initial_prompt'),
 'cancel_both':(1,3,TIMEOUT+EARLY,'cancel_both_prompt'),
 'vertical_length':(3,3,TIMEOUT,'vertical_length_finish'),
 'vertical_negative':(3,3,TIMEOUT,'vertical_negative_finish'),
 'vertical_both':(4,4,TIMEOUT,'vertical_both_finish'),
 'bisector_length':(5,5,TIMEOUT,'bisector_length_finish'),
 'angled_length':(5,5,TIMEOUT,'angled_length_finish'),
 'native_epsilon_below':(2,3,EARLY,'epsilon_below_complete'),
 'bisector_opposite':(4,5,EARLY,'opposite_complete'),
 'tiny_1e12':(2,3,EARLY,'tiny_complete_1e12'),
 'tiny_1e10':(2,3,EARLY,'tiny_complete_1e10'),
 'tiny_zero_tolerance':(2,3,EARLY,'tiny_complete_zero_tolerance'),
 'tiny_1e9':(2,3,EARLY,'tiny_complete_1e9'),
 'tiny_repeat_then_end':(2,3,EARLY,'tiny_complete_1e12'),
}
INVALID={'tiny_complete_1e12','tiny_complete_1e10','tiny_complete_zero_tolerance'}
ALL_INVALID=INVALID|{'tiny_1e12','tiny_1e10','tiny_zero_tolerance','tiny_repeat_then_end'}

def observations():
    return {r['id']:r['value'] for f in FAMILIES for r in json.loads(gzip.decompress((HERE/('observations/'+f+'.json.gz')).read_bytes()))['results']}

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '') is None:raise ValueError('full implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    sha=lambda b:hashlib.sha256(b).hexdigest();checks=json.loads((proof/'checks.json').read_text())
    if [(c['name'],c['command']) for c in checks]!=list(commands()) or any(c['status'] for c in checks):raise ValueError('incomplete or failed checks')
    receipts=json.loads((proof/'captures.json').read_text())
    if len(receipts)!=4 or {r['family'] for r in receipts}!=set(FAMILIES):raise ValueError('native receipts differ')
    from .line_construction_cases import CASES,request
    sidecar=HERE/'fixtures/line_construction_sources.json'
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
        if receipt['exit_status']!=0 or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family] or log.splitlines()!=['Capturing '+str(count)+' cases: '+family,'Recorded '+family]:raise ValueError('native capture incomplete')
        captures.append(dict(family=family,observations=count,raw_sha256=sha(raw),engine_version=data['engine_version']))
        for row in data['results']:
            k,v=row['id'],row['value'];t=v['input_trace'][0] if v['input_trace'] else None
            if k in native or not v['returned_idle'] or v['created']!=v['after'] or v['after']!=v['redo'] or v['before']!=v['undo']:raise ValueError('native completion or history differs')
            if 'Unknown command' in v['history'] and k!='both_repeat':raise ValueError('unexpected native recovery')
            if CASES[k].get('macro'):
                if t is not None:raise ValueError('unexpected macro input driver')
            elif k in INCOMPLETE:
                n,expected,errors,corrected=INCOMPLETE[k]
                if t['completed']!=n or t['expected']!=expected or t['errors']!=errors:raise ValueError('driver diagnostic changed')
                diagnostics.append(dict(id=k,kind='incomplete or timed-out owned input',corrected_by=corrected,qualified=False))
            elif t['errors'] or t['completed']!=t['expected']:raise ValueError('incomplete native input')
            valid=all(o['native_valid'] for o in v['after'])
            if valid!=(k not in ALL_INVALID):raise ValueError('native-invalid scope changed')
            if k in INVALID:diagnostics.append(dict(id=k,kind='native SDK marks a created line invalid',coefficients_compared=True,qualified=False))
            native[k]=v
    if set(native)!=set(CASES) or len(native)!=68:raise ValueError('native scope changed')
    complete={k:v for k,v in native.items() if k not in INCOMPLETE}
    kernel={k for k,v in complete.items() if v['success'] and k not in {'repeated_point','both_repeat'} and not CASES[k].get('macro')}
    if len(complete)!=54 or len(kernel)!=40 or sum(v['success'] for v in complete.values())!=43:raise ValueError('native construction scope changed')
    for k,(_,_,errors,corrected) in INCOMPLETE.items():
        if errors==EARLY and (native[k]['after']!=native[corrected]['after'] or native[k]['success']!=native[corrected]['success']):raise ValueError('corrected native workflow changed')
    cases={k:dict(id=k,success=v['success'],native_valid=k not in INVALID,qualified=k not in INVALID) for k,v in complete.items()}
    for mode in ['portable','native']:
        for stage,expected in [('ui',set(complete)),('command',kernel)]:
            text=gzip.decompress((proof/('line-'+stage+'-'+mode+'.log.gz')).read_bytes()).decode()
            rows=[json.loads(s) for s in re.findall(r'^LINE_'+stage.upper()+r' (.+)$',text,re.M)]
            if len(rows)!=len(expected) or {r['id'] for r in rows}!=expected:raise ValueError('incomplete or duplicate Line replay')
            for row in rows:
                k=row['id'];error=row['maximum_coefficient_and_sample_error']
                if type(error) not in [int,float] or not math.isfinite(error) or not 0<=error<=1e-6:raise ValueError('geometry gate failed: '+k)
                if row['native_valid'] is not (k not in INVALID):raise ValueError('native-validity replay scope changed')
                if row['archives']!=int(complete[k]['success'] and k not in INVALID):raise ValueError('archive coverage differs')
                cases[k][mode+'_'+stage+'_error']=error
    sources=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    if not {'crates/viboceros-command/src/line.rs','src/app/line_input.rs','src/viewport.rs'}.issubset(sources):raise ValueError('revision lacks the Line implementation')
    return dict(scope='Line BothSides, Vertical, FourPoint, Bisector and Angled; shared edit-free getters, axis picking and preview.',baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,
        full_native_parity=False,continuous_native_distance_qualified=False,performance_qualified=False,native_observations=68,complete_native_inputs_per_build=54,qualified_valid_native_workflows_per_build=51,
        valid_getter_or_file_constructions_per_build=40,cancel_controls_per_build=11,kernel_constructions_per_build=40,valid_kernel_constructions_per_build=37,archive_round_trips_per_build=77,
        maximum_coefficient_and_sample_difference=max(v for c in cases.values() for k,v in c.items() if k.endswith('_error')),
        captures=captures,cases=list(cases.values()),diagnostics=diagnostics,checks=[dict(name=c['name'],status=c['status']) for c in checks],
        execution='Native Rhino ran in private Xvfb sessions. Rust application tests opened no desktop windows.',
        limits=['Fourteen initial probes timed out or supplied extra tokens after native completion. They remain unqualified driver diagnostics; fresh captures cover corrected complete sequences.',
        'Three complete tiny-line outputs are native SDK-invalid. Their coefficients, parameter domains, metadata and history are compared, with native-valid geometry and archives explicitly unqualified.',
        'Curve controls, weights, knots and 33 normalized-parameter samples are checked. Samples evaluate captured definitions locally; independent native PointAt sampling and continuous Hausdorff distance are unqualified.',
        'Typed native points qualify construction geometry. Windowless synthetic viewport clicks check Vertical in Front and Perspective, preview/commit agreement, and atomic history; arbitrary mouse trajectories are unqualified.',
        'Normal, Tangent, Perpendicular, Extension and comparative performance remain unimplemented or unqualified.'],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in sources},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(evidence)})

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);args=p.parse_args()
    (ROOT/'docs/line-construction-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
