"""Pin open Chamfer contacts, consumption, exact inputs and actual history."""
import argparse
import gzip
import json
import re
import subprocess
from .check_chamfer_open import ROOT, PROOF, commands
from .chamfer_open_evidence import BASELINE, INVALID, collect, result, sha
from .report_fillet_zero import gate


def comparisons(proof):
    rows,captures,counts,paths,legacy = collect(proof)
    cases = {key:dict(id=key,native_result=result(v),native_valid=key not in INVALID,
                     parity_qualified=key not in INVALID) for key,v in rows.items()}
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            expected = {key for key,v in rows.items() if key not in INVALID and (stage == 'ui' or result(v) == 'Success')}
            text = gzip.decompress((proof/('open-'+stage+'-'+mode+'.log.gz')).read_bytes()).decode()
            marker = 'CHAMFER_OPEN_'+stage.upper()+' '; seen = set()
            for line in text.splitlines():
                if not line.startswith(marker): continue
                row = json.loads(line[len(marker):]);key = row['id'];error = row['maximum_geometry_error'];gate(error)
                if key not in expected or key in seen: raise ValueError('incomplete or duplicate comparison')
                if row['archives'] != 1 or row['changed'] is not rows[key]['history_changed'] or row['success'] is not (result(rows[key]) == 'Success'): raise ValueError('history or archive gate')
                cases[key][mode+'_'+stage+'_maximum_geometry_error'] = error;seen.add(key)
            if seen != expected: raise ValueError('incomplete or duplicate comparison')
            if 'test result: ok. '+str(1 if stage == 'command' else 2)+' passed; 0 failed;' not in text: raise ValueError('failed tests')
            if stage == 'ui':
                if text.count('CHAMFER_OPEN_MOUSE_SOURCE_PICKS 2') != 1: raise ValueError('missing mouse source picks')
                guards = [json.loads(line.split(' ',1)[1]) for line in text.splitlines() if line.startswith('CHAMFER_OPEN_INVALID_UI ')]
                if len(guards) != len(INVALID) or {r['id'] for r in guards} != INVALID: raise ValueError('incomplete invalid UI guards')
                for row in guards:
                    gate(row['maximum_geometry_error'])
                    if row['archives'] != 1 or row['redo_preserved'] is not True or row['native_invalid'] is not True: raise ValueError('invalid UI preservation')
                    cases[row['id']][mode+'_original_archive_maximum_geometry_error'] = row['maximum_geometry_error']
        text = gzip.decompress((proof/('open-command-'+mode+'.log.gz')).read_bytes()).decode()
        guards = [json.loads(line.split(' ',1)[1]) for line in text.splitlines() if line.startswith('CHAMFER_OPEN_REJECTION ')]
        expected = {key for key,v in rows.items() if result(v) == 'Failure'} | INVALID
        if len(guards) != len(expected) or {r['id'] for r in guards} != expected or any(r['redo_preserved'] is not True or r['native_invalid'] is not (r['id'] in INVALID) for r in guards): raise ValueError('incomplete API rejection guards')
    for key in INVALID: cases[key].update(product_rejection_qualified=True,reason='Native creates an invalid disconnected PolyCurve; product preserves original geometry, metadata and Redo.')
    return cases,captures,counts,paths,legacy


def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '') is None: raise ValueError('full implementation revision required')
    git = lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks = json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command']) for c in checks] != list(commands()) or any(c['status'] for c in checks): raise ValueError('incomplete checks')
    cases,captures,counts,paths,legacy = comparisons(proof)
    changed = git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required = {'crates/viboceros-command/src/chamfer/open.rs','crates/viboceros-command/src/chamfer/open/setbacks.rs','crates/viboceros-command/src/chamfer/open/setbacks/overflow.rs','crates/viboceros-command/src/chamfer/open/setbacks/smooth.rs','crates/viboceros-command/src/curve_pair_edit/line_contacts.rs','crates/viboceros-command/src/curve_pair_edit/line_supports.rs','crates/viboceros-command/tests/support/chamfer_getter.rs','src/app/tests/chamfer_open.rs','tools/rhino_oracle/chamfer_command_probe.py'}
    if not required.issubset(changed): raise ValueError('missing contact and consumption implementation')
    return dict(scope='Distinct open-curve Chamfer contacts, picked crossing remainders, zero and oversized setbacks, complete consumption and actual Undo/Redo.',
        baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,**counts,
        valid_native_workflows=65,successful_command_replays_per_build=58,qualified_ui_replays_per_build=65,
        invalid_native_ui_guards_per_build=4,api_rejections_per_build=10,archive_round_trips_per_build=128,
        windowless_source_picks_per_build=2,model_epsilon=1e-6,
        maximum_geometry_difference=max(v for c in cases.values() for k,v in c.items() if k.endswith('_maximum_geometry_error')),
        construction='Pick travel ranks contact candidates; setbacks follow the picked side. Finite setbacks use endpoint sphere crossings. Complete consumption preserves separate sources and omits consumed pieces from joins. Oversized line replacements reflect the cut parameter about the original near endpoint while bevels retain their actual cut points. Circular setbacks can wrap into a major arc. Tangent NURBS consumption finds the bevel on the far tangent and continues the retained near end. Smooth overlaps retain individual 50-span supports; positive-distance bevels use 50 pair-bounds diagonals per end. Shared contact supports retain original charts. Explicit source replacements create one history step even when geometry is equal.',
        execution='Five immutable licensed-Rhino captures ran on private Xvfb displays. All 69 observations are accounted for: 65 valid outcomes are replayed, and four invalid Native joins receive atomic preservation guards. Both application builds ran without windows.',
        input_evidence='Authored curves, pre-command physical sight points and accepted numeric tokens are inputs. Native coefficients, domains, 33 point/tangent stations, metadata and UndoRedo events are expectations only. Native prompt display rounding does not replace exact accepted numeric input.',
        history_evidence='Native public UndoRedo begin/end pairs identify 61 records and 59 ordered scene changes, including two equal-geometry replacements. The legacy zero-distance fixture retains its original raw archive and receives a derived history expectation from its matching modern event-observed case. The formerly rejected oversized fixture now matches its fresh native repeat.',
        legacy_correspondence=legacy,
        limits=['Four Native successful joined results are invalid disconnected PolyCurves. Their public diagnostics remain archived; product rejection and original-geometry preservation are qualified, with no invalid-output geometry-parity claim.', 'Finite coverage does not qualify arbitrary folded or singular curves, all intersection branches, tied pick costs, arbitrary parameterizations, tiny geometric limits, continuous distance bounds, pixel equality or comparative performance. Same-parent and closed-source adapters retain separate qualifications.'],
        cases=[cases[k] for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(paths)})


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--implementation-revision',required=True);args=parser.parse_args()
    (ROOT/'docs/chamfer-open-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')


if __name__ == '__main__': main()
