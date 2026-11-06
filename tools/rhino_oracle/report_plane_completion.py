"""Rebuild Plane Enter qualification from frozen captures and executed checks."""
import argparse
import gzip
import hashlib
import json
import math
from pathlib import Path
import re
import subprocess
from .check_plane_completion import ROOT, PROOF, commands

HERE=Path(__file__).parent
BASELINE='a8765d384a948a04850242bb3088b9f7ee60af09'
FAMILIES={'plane_finish_discovery':10,'plane_finish_workflows':37,'plane_finish_pointer':41,
          'plane_completion':53,'plane_completion_terminal':53}
RECOVERED={'pointer_deform_empty','pointer_deform_repeated','pointer_fractional_degree'}

def build(proof, revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '') is None: raise ValueError('full implementation revision required')
    git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}')
    sha=lambda b:hashlib.sha256(b).hexdigest()
    checks=json.loads((proof/'checks.json').read_text())
    if [(c['name'],c['command']) for c in checks]!=list(commands()) or any(c['status'] for c in checks): raise ValueError('incomplete or failed checks')
    evidence=set(proof.iterdir());captures=[];native=None
    runs=json.loads((proof/'captures.json').read_text())
    if {r['family'] for r in runs}!=set(FAMILIES): raise ValueError('missing capture receipts')
    for family,count in FAMILIES.items():
        path=HERE/('observations/'+family+'.json.gz');raw=gzip.decompress(path.read_bytes());data=json.loads(raw)
        fixture=HERE/('fixtures/'+family+'.json');query=json.loads(fixture.read_bytes())
        if len(data['results'])!=count or [r['id'] for r in data['results']]!=[o['id'] for o in query['operations']]: raise ValueError('native coverage differs')
        if data['provenance']['request_sha256']!=sha(fixture.read_bytes()): raise ValueError('native recipe changed')
        evidence.update([path,fixture])
        for digest in data['provenance']['sources'].values():
            archive=HERE/('observations/source_snapshots/'+digest+'.gz')
            if sha(gzip.decompress(archive.read_bytes()))!=digest: raise ValueError('producer archive changed')
            evidence.add(archive)
        run=next(r for r in runs if r['family']==family)
        log=gzip.decompress((proof/run['log']).read_bytes()).decode()
        if run['command'][:2]!=['tools/rhino_oracle/run_headless.sh','exec'] or 'Recorded '+family not in log or run['exit_status'] not in [None,0]: raise ValueError('capture did not publish')
        captures.append(dict(family=family,observations=count,raw_sha256=sha(raw),engine_version=data['engine_version'],role='qualification' if family=='plane_completion_terminal' else 'exploration'))
        if family=='plane_completion_terminal': native={r['id']:r['value'] for r in data['results']}
    if len(native)!=53 or sum(v['success'] for v in native.values())!=42: raise ValueError('native scope changed')
    recovered={k for k,v in native.items() if 'Unknown command' in v['history']}
    if recovered!=RECOVERED: raise ValueError('unexpected native recovery')
    for v in native.values():
        t=v['input_trace'][0]
        if t['completed']!=t['expected'] or t['errors'] or not v['returned_idle']: raise ValueError('incomplete native input')
        if v['pointer']['pixel']!=v['pointer_after']['pixel']: raise ValueError('pointer moved during recipe')
    scripts={k for k,v in native.items() if v['success'] and k not in recovered}
    cases={k:dict(id=k,success=v['success'],recovered_invalid_option=k in recovered) for k,v in native.items()}
    for mode in ['portable','native']:
        def rows(name,prefix):
            text=gzip.decompress((proof/(name+'-'+mode+'.log.gz')).read_bytes()).decode()
            rows=[json.loads(s) for s in re.findall('^'+prefix+r' (.+)$',text,re.M)]
            if len({r['id'] for r in rows})!=len(rows): raise ValueError('duplicate replay row')
            return {r['id']:r for r in rows}
        cli=rows('commands','PLANE_COMPLETION_SCRIPT');ui=rows('ui','PLANE_COMPLETION_UI')
        if set(cli)!=scripts or set(ui)!=set(native): raise ValueError('incomplete CLI or UI execution')
        for k,v in native.items():
            for route,result in [('script',cli.get(k)),('ui',ui[k])]:
                error=None if result is None else result['maximum_coefficient_and_sample_error']
                if error is not None and (type(error) not in [int,float] or not math.isfinite(error) or not 0<=error<=1e-6): raise ValueError('geometry gate failed: '+k)
                cases[k][mode+'_'+route+'_error']=error
            if ui[k]['archive_objects']!=(len(v['after']) if v['success'] else None): raise ValueError('archive coverage differs')
            cases[k]['archive_objects']=ui[k]['archive_objects']
    sources=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    return dict(scope='Plane Enter defaults, live pointer direction/orientation, Deformable actions and integer input.',
        baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,
        full_native_parity=False,continuous_native_distance_qualified=False,performance_qualified=False,
        native_observations=sum(FAMILIES.values()),qualified_workflows=53,script_constructions=len(scripts),
        ui_constructions=42,ui_controls=11,recovered_invalid_option_workflows=3,archive_round_trips_per_build=42,
        maximum_coefficient_and_sample_difference=max(value for case in cases.values() for key,value in case.items() if key.endswith('_error') and value is not None),
        parameter_grid=[9,9],captures=captures,cases=list(cases.values()),
        execution='Rhino captures used private Xvfb displays; command, egui and application tests created no windows.',
        limits=['The preceding 141 observations are exploratory, including unused Cancel tails, incomplete recipes, and ClientToWorld lines whose direction was not a forward camera ray.',
            'The final 53 recipes consume every planned token and record stable cursor pixels before and after. Pointer rays and initial setting prompts are independent input context, never inferred from constructed geometry.',
            'CLI replays execute 39 accepted constructions with camera/pointer context. Three invalid-option recoveries and eleven cancellations are interactive-only.',
            'Native cancellation undo/redo fields are passive snapshots. Local cancellation tests separately check pending Redo preservation.',
            'GUI native replay supplies drafting hover from the saved camera ray and current getter plane; separate windowless egui tests check stationary-pointer transitions and full command-line execution.',
            'These recipes cover Top, Front and Perspective views and rotated construction planes. General mouse trajectories, all snap modes, degenerate camera directions, curved guide and surface-edge parity, restart persistence and speed remain unqualified.'],
        checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in sources},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(evidence) if p.is_file()})

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
    (ROOT/'docs/plane-completion-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
