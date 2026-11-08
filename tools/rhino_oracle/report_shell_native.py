"""Pin finite Shell comparisons, complete capture accounting and implementation."""
import argparse
import gzip
import json
import math
import re
import subprocess
from .check_shell_native import ROOT, PROOF, commands
from .shell_native_evidence import BASELINE, DIAGNOSTIC, collect, result, sha


def gate(value, maximum):
    if type(value) not in (int,float) or not math.isfinite(value) or not 0<=value<=maximum:
        raise ValueError('geometry gate')


def portable(rows):
    names = set()
    for key,v in rows.items():
        if key==DIAGNOSTIC: continue
        thickness = float(v['input_trace'][0]['prompts'][0].split('Thickness=')[1].split()[0])
        for action in v['prescribed_case']['actions']:
            if type(action) in (int,float) and action>=0: thickness=action
            if isinstance(action,dict) and 'number' in action:
                token=action['number']
                if not token.endswith('cm'): raise ValueError('unexpected unit recipe')
                thickness=float(token[:-2])*10
        if result(v)!='Success' or thickness==0: names.add(key)
    if len(names)!=12: raise ValueError('portable inventory')
    return names


def comparisons(proof=PROOF):
    rows,sdk,signed,captures,counts,paths=collect(proof)
    expected=set(rows)-{DIAGNOSTIC}; cases={key:dict(native_result=result(v),qualified=key!=DIAGNOSTIC) for key,v in rows.items()}
    cases[DIAGNOSTIC]['reason']='The discovery used an inline Thickness macro, entered its subgetter, rejected DeleteInput there and timed out; it qualifies no admission.'
    for mode in ['portable','native']:
        names=portable(rows) if mode=='portable' else expected
        for stage in ['command','ui']:
            log=proof/(stage+'-'+mode+'.log.gz'); text=gzip.decompress(log.read_bytes()).decode(); seen=set()
            for line in text.splitlines():
                if not line.startswith('SHELL_'+stage.upper()+' '): continue
                row=json.loads(line.split(' ',1)[1]);key=row['id'];gate(row['maximum_geometry_error'],1e-6)
                if key in seen or key not in names: raise ValueError('incomplete or duplicate comparison')
                if (row['archives']!=1 or row['changed'] is not rows[key]['history_changed']
                        or row['success'] is not (result(rows[key])=='Success')): raise ValueError('history or archive gate')
                seen.add(key);cases[key][stage+'_'+mode+'_maximum_geometry_error']=row['maximum_geometry_error']
            if seen!=names or text.count('SHELL_'+stage.upper()+'_COUNT '+str(len(names)))!=1: raise ValueError('incomplete or duplicate comparison')
            passed=7 if stage=='ui' else 2 if mode=='portable' else 3
            if f'test result: ok. {passed} passed; 0 failed; 0 ignored;' not in text: raise ValueError('failed comparison tests')
            if stage=='ui' and text.count('SHELL_MOUSE_FACE_PICKS 2')!=1: raise ValueError('missing pointer controls')
            paths.add(log)
    sdk_rows={}; text=gzip.decompress((proof/'command-native.log.gz').read_bytes()).decode()
    for line in text.splitlines():
        if not line.startswith('SHELL_SDK '): continue
        row=json.loads(line.split(' ',1)[1]);key=row['id'];gate(row['geometry_gate'],2.001e-6)
        gate(row['maximum_geometry_error'],row['geometry_gate']);gate(row['volume_gate'],1.)
        gate(row['volume_error'],row['volume_gate'])
        if key not in sdk or key in sdk_rows: raise ValueError('incomplete or duplicate SDK comparison')
        expected_topology=[[len(b['geometry'][part]) for part in ['vertices','edges','faces']]
                           for b in sdk[key]['breps']]
        if row['native_topology']!=expected_topology: raise ValueError('native SDK topology receipt')
        if not key.startswith('shell_cylinder_') and row['native_topology']!=row['actual_topology']: raise ValueError('box topology gate')
        sdk_rows[key]=row
    if set(sdk_rows)!=set(sdk) or text.count('SHELL_SDK_COUNT 9')!=1: raise ValueError('incomplete or duplicate SDK comparison')
    return cases,sdk_rows,captures,counts,paths


def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '') is None: raise ValueError('full implementation revision required')
    git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command']) for c in checks]!=list(commands()) or any(c['status'] for c in checks): raise ValueError('incomplete checks')
    cases,sdk,captures,counts,paths=comparisons(proof);paths.update(p for p in proof.iterdir() if p.is_file())
    changed=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required={'crates/viboceros-command/src/shell/zero.rs','crates/viboceros-command/src/shell/options.rs',
        'crates/viboceros-geometry/src/brep/trim_scan.rs','crates/viboceros-command/tests/shell_native.rs',
        'src/app/shell.rs','src/app/tests/shell/mouse.rs'}
    if not required.issubset(changed): raise ValueError('missing Shell implementation')
    return dict(scope='Shell thickness getters, remembered options, zero-thickness topology and native document admission.',
        baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,
        continuous_error_certificate=False,performance_qualified=False,**counts,
        native_admissions=17,native_no_admission_controls=7,portable_admissions=5,portable_no_admission_controls=7,
        command_and_ui_archive_round_trips=dict(native=48,portable=24),windowless_pointer_picks_per_build=2,
        getter_epsilon=1e-6,sdk_epsilon=2e-6,
        maximum_getter_geometry_difference=max(v for c in cases.values() for k,v in c.items() if k.endswith('_maximum_geometry_error')),
        maximum_sdk_geometry_difference=max(v['maximum_geometry_error'] for v in sdk.values()),
        execution='Five immutable licensed Rhino captures ran on private Xvfb displays. All 40 observations are retained: 25 command outcomes, nine geometry SDK cases and six two-sign SDK discoveries. The command discovery with an invalid inline macro remains diagnostic. Five earlier failed attempts retain logs, requests and frozen producers; their temporary native responses were not retained and qualify no geometry. Rust UI and pointer checks ran without windows.',
        inputs='Product sources use authored boxes, cylinder dimensions and opening points. Accepted numeric values and source face picks drive replays. Initial public prompts seed provisional preferences. Native output coefficients and topology serve only as expectations.',
        comparison='Bidirectional tolerance-aware trimmed-face grids and trim stations, oriented normals at regular chart sites, edge stations and vertices where partitions correspond, solid flags, wall volume, bounds, source identity, names, layers, colors, groups, selection, actual Undo/Redo and 3DM round trips. Getter boundary differences use 1e-6. SDK differences use the combined 2e-6 allowance for two independently requested 1e-6 approximations plus 64 machine epsilons at coordinate scale; volume uses boundary area times model tolerance.',
        admission='DeleteInput=Yes retains source identity for the first material region; No copies source attributes and groups. Outputs are unselected. Zero thickness doubles the retained skins with opposite normals, shares the opening rim and duplicates interior edges. The closed topology has zero material volume and is available in both builds.',
        repairs='The public SDK recipe uses positive thickness for inward walls; the kernel adapter keeps its own negative offset convention. Exported roundoff UV joints use continuous edge-deviation certificates. Scan overlaps and incident endpoint events now use a consistent parameter slab, fixing a reproduced remote ring-membership error.',
        limits=['Circular SDK walls have different face partitions and polar cap charts between the kernels. Their oriented boundary geometry and mass are compared; matching internal seams, face/edge/vertex counts and pole normals are not claimed.',
            'Portable positive-thickness Shell still requires the native kernel. General self-intersections, arbitrary singular inputs, near-collapsed material, disconnected zero-thickness partition policy, new-piece identity/order, rendering materials, unobserved user data, History EditThickness, live preview and comparative speed remain unqualified.',
            'The finite native getter recipes cover numeric thickness, one explicit unit suffix, Enter/Escape, physical source picks, replacement/copy admission and two-object batches. Preselection convenience, inline API options, factory preferences, arbitrary unit systems and point-measured thickness do not have complete native getter parity.'],
        cases=[dict(id=k,**cases[k]) for k in sorted(cases)],sdk_cases=[sdk[k] for k in sorted(sdk)],captures=captures,
        checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(paths)})


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);args=p.parse_args()
    (ROOT/'docs/shell-native-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')


if __name__=='__main__': main()
