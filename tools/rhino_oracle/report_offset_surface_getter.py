"""Pin OffsetSrf's finite native comparisons and complete source/evidence inventory."""
import argparse
import gzip
import json
import math
import re
import subprocess
from .check_offset_surface_getter import ROOT, PROOF, commands
from .offset_surface_getter_evidence import BASELINE, DIAGNOSTICS, collect, result, sha

def gate(value):
    if not isinstance(value,(int,float)) or not math.isfinite(value) or not 0<=value<=1e-6: raise ValueError('geometry gate')

def comparisons(proof):
    rows,captures,counts,paths=collect(proof)
    expected=set(rows)-set(DIAGNOSTICS)
    cases={key:dict(id=key,native_result=result(v),qualified=key in expected) for key,v in rows.items()}
    for key,reason in DIAGNOSTICS.items():cases[key]['reason']=reason
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            text=gzip.decompress((proof/(stage+'-'+mode+'.log.gz')).read_bytes()).decode();seen=set()
            marker='OFFSET_SURFACE_'+stage.upper()+' '
            for line in text.splitlines():
                if not line.startswith(marker):continue
                row=json.loads(line[len(marker):]);key=row['id'];gate(row['maximum_geometry_error'])
                if key not in expected or key in seen:raise ValueError('incomplete or duplicate comparison')
                if row['archives']!=1 or row['changed'] is not rows[key]['history_changed'] or row['success'] is not (result(rows[key])=='Success'):raise ValueError('history or archive gate')
                seen.add(key);cases[key][mode+'_'+stage+'_maximum_geometry_error']=row['maximum_geometry_error']
            if seen!=expected or text.count('OFFSET_SURFACE_'+stage.upper()+'_COUNT 47')!=1:raise ValueError('incomplete or duplicate comparison')
            passed=2 if stage=='command' else 4 if mode=='portable' else 5
            if f'test result: ok. {passed} passed; 0 failed; 0 ignored;' not in text:raise ValueError('failed tests')
            control='OFFSET_SURFACE_ATOMIC_GUARDS 10' if stage=='command' else 'OFFSET_SURFACE_MOUSE_FACE_PICKS 4'
            if text.count(control)!=1:raise ValueError('missing atomic or pointer controls')
    return cases,captures,counts,paths

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '') is None:raise ValueError('full implementation revision required')
    git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command']) for c in checks]!=list(commands()) or any(c['status'] for c in checks):raise ValueError('incomplete checks')
    cases,captures,counts,paths=comparisons(proof);paths.update(p for p in proof.iterdir() if p.is_file())
    changed=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required={'crates/viboceros-command/src/offset_surface/prepare.rs','crates/viboceros-command/src/offset_surface/options.rs','crates/viboceros-command/src/offset_surface/zero.rs','crates/viboceros-command/src/offset_surface/through_point.rs','crates/viboceros-command/tests/offset_surface_getter.rs','src/app/offset_surface.rs','src/app/offset_surface/input.rs','src/app/tests/offset_surface/mouse.rs'}
    if not required.issubset(changed):raise ValueError('missing offset getter implementation')
    return dict(scope='Sharp-corner OffsetSrf selection, distance and direction editing, through-point acceptance and document admission.',
        baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,**counts,
        command_admissions_per_build=39,command_no_admission_controls_per_build=8,ui_workflows_per_build=47,
        archive_round_trips_per_build=94,windowless_pointer_picks_per_build=4,atomic_api_controls_per_build=10,
        retained_failed_import_attempts=1,model_epsilon=1e-6,
        maximum_geometry_difference=max(v for c in cases.values() for k,v in c.items() if k.endswith('_maximum_geometry_error')),
        execution='Eight immutable licensed Rhino captures ran on private Xvfb displays. All 56 outcomes are accounted for: 47 complete Sharp workflows are compared, while nine discovery or unsupported-mode observations remain separate. The failed IronPython import retains its request, frozen sources and host error; its temporary native response was not retained and it qualifies no geometry. Both product builds ran without windows.',
        input_evidence='Authored corners, rectangular trims, source orientation and accepted numeric/point inputs construct product replays. Initial public prompts seed provisional preferences; exact subsequent authored values control accepted offsets. Native output surfaces, curves, topology and document rows are expectations only. One acknowledged native physical click identifies a source flip; product pointer controls exercise source selection and flipping in wireframe and shaded views.',
        comparison='Finite bidirectional trimmed-face grid and trim stations, oriented normals where defined, edge stations, vertices, bounds, face/edge/vertex and trim-kind counts, names, layers, colors, groups, source identity, actual Undo/Redo and 3DM round trips. Command cancellation controls prepare without admission; UI controls replay the getters.',
        admission='Copies use current-layer attributes and retain preselected sources. Open BothSides emits two objects. Nonzero Solid/DeleteInput replaces source geometry in place. Zero one-sided walls emit two coincident caps and collapsed ruled sheets; DeleteInput copies source attributes/groups onto those pieces and removes the source. Zero two-sided walls form one oppositely oriented two-face B-rep and retain source identity when replacing it.',
        limits=['Only Sharp corner mode is implemented. The completed Round pilot remains unqualified; Loose and explicit tolerance options remain unsupported.', 'Native factory preferences are not established by private-profile prompts. Rendering materials and other unobserved user data are unqualified.', 'Finite planar rectangular, trimmed, tilted, reversed and two-object through-point cases do not establish arbitrary curved closest-point branches, singular inputs, general solid fallback partitions, all subobject workflows, selection-distance ties, new-object creation order, continuous geometric bounds, pixel equality or comparative performance. Existing analytic and general-kernel geometry retain their separate qualifications.'],
        cases=[cases[k] for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(paths)})

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);args=p.parse_args()
    (ROOT/'docs/offset-surface-getter-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')

if __name__=='__main__':main()
