"""Pin zero-radius Fillet geometry, history, preview and archive evidence for open curves."""
import argparse
import gzip
import json
import re
import subprocess
from .check_fillet_open_zero import ROOT, PROOF, commands
from .fillet_open_zero_evidence import BASELINE, collect, result, sha
from .report_fillet_zero import gate


def comparisons(proof):
    rows, captures, counts, paths, corrections = collect(proof)
    cases = {key:dict(id=key, native_result=result(v), native_valid=True)
             for key,v in rows.items()}
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            expected = {key for key,v in rows.items() if stage == 'ui' or result(v) == 'Success'}
            text = gzip.decompress((proof/('open-zero-'+stage+'-'+mode+'.log.gz')).read_bytes()).decode()
            marker = 'FILLET_OPEN_ZERO_'+stage.upper()+' '; seen = set()
            for line in text.splitlines():
                if not line.startswith(marker): continue
                row = json.loads(line[len(marker):]); key = row['id']; error = row['maximum_geometry_error']
                if key not in expected or key in seen: raise ValueError('incomplete or duplicate comparison')
                gate(error)
                if row['archives'] != 1 or row['changed'] is not rows[key]['history_changed'] or row['success'] is not (result(rows[key]) == 'Success'): raise ValueError('history or archive gate')
                cases[key][mode+'_'+stage+'_maximum_geometry_error'] = error; seen.add(key)
            if seen != expected: raise ValueError('incomplete or duplicate comparison')
            if 'test result: ok. 2 passed; 0 failed;' not in text: raise ValueError('failed tests')
            if stage == 'ui' and text.count('FILLET_OPEN_ZERO_MOUSE_SOURCE_PICKS 2') != 1: raise ValueError('missing mouse routing')
        text = gzip.decompress((proof/('open-zero-command-'+mode+'.log.gz')).read_bytes()).decode()
        guards = [json.loads(line.split(' ',1)[1]) for line in text.splitlines() if line.startswith('FILLET_OPEN_ZERO_REJECTION ')]
        expected = {key for key,v in rows.items() if result(v) == 'Failure'}
        if len(guards) != len(expected) or {r['id'] for r in guards} != expected or any(r['redo_preserved'] is not True or r['native_invalid'] is not False for r in guards): raise ValueError('incomplete API rejection guards')
    producer = gzip.decompress((proof/'native-producers.log.gz').read_bytes()).decode()
    if producer.count('BOUNDED_LINE_OVERLAPS 64') != 1: raise ValueError('missing bounded native producer regression')
    return cases, captures, counts, paths, corrections


def build(proof, revision):
    if re.fullmatch('[0-9a-f]{40}', revision or '') is None: raise ValueError('full implementation revision required')
    git = lambda *args: subprocess.check_output(['git',*args],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}'); git('merge-base','--is-ancestor',BASELINE,revision)
    checks = json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command']) for c in checks] != list(commands()) or any(c['status'] for c in checks): raise ValueError('incomplete checks')
    cases, captures, counts, paths, corrections = comparisons(proof)
    changed = git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required = {'crates/viboceros-command/src/fillet/open.rs', 'crates/viboceros-command/src/fillet/zero.rs', 'crates/viboceros-command/src/fillet/zero/lines.rs', 'crates/viboceros-command/src/fillet/zero/admission.rs', 'crates/viboceros-command/tests/fillet_open_zero.rs', 'crates/viboceros-command/src/fillet/prepare.rs', 'crates/viboceros-command/tests/support/fillet_getter/history.rs', 'src/app/tests/fillet_open_zero.rs', 'crates/viboceros-smlib/native/bridge.cpp', 'crates/viboceros-smlib/tests/curve_intersections/bounded_lines.rs'}
    if not required.issubset(changed): raise ValueError('missing open zero implementation')
    return dict(scope='Zero-radius Fillet for two distinct open curves, bounded line overlaps, picked remainders and actual Undo/Redo.',
        baseline_revision=BASELINE, implementation_revision=revision, full_native_parity=False, **counts,
        successful_command_replays_per_build=45, qualified_ui_replays_per_build=49,
        api_rejections_per_build=3, archive_round_trips_per_build=95,
        native_bounded_line_producer_cases=64,
        windowless_source_picks_per_build=2, model_epsilon=1e-6,
        maximum_geometry_difference=max(v for c in cases.values() for k,v in c.items() if k.endswith('_maximum_geometry_error')),
        construction='Distinct pairs share zero-radius contact selection and admission. Trim=No is an immediate no-op; Trim=Yes retains the picked side of each open source. Curved continuations use the public extension style. Line-mode supports are padded by 100 times the pair bounding-box diagonal per end; Smooth line supports by 50 source spans. Summed physical pick travel ranks overlap contacts. Smooth collinear contacts retain separate support endpoints, averaged on Join while preserving their parameter charts. Equal source replacements explicitly record one history step. Preview and commit reuse one prepared plan. The native intersection bridge permits endpoint roundoff within 64 floating-point epsilons of its parameter scale, while independently checking source-space residuals and overlap interiors.',
        execution='Five successful private-Xvfb captures publish all 49 observations, all with valid geometry. The failed overlap capture and unusable private selection-menu intervention publish no observations; its complete eleven-case request is repeated with separated long-line picks. Portable and native application tests run without windows.',
        input_evidence='Authored source definitions and pre-command physical sight points are inputs. Native coefficients, domains, 33 point/tangent stations, attributes and history events are expectations only.',
        history_evidence='Public Command.UndoRedo begin/end pairs distinguish 42 records from seven no-ops, cancellations or failures. Three records have unchanged ordered scene values. Two older getter fixtures retain their original raw archives and receive derived history expectations from a matching modern event-observed workflow.',
        legacy_history_corrections=corrections,
        limits=['Finite line, arc and NURBS cases qualify these observations only. Arbitrary overlaps, tied pick costs, other parameterizations, folded or singular continuations, arbitrary nonplanar curves, tiny positive radii, continuous distance bounds, pixel equality and comparative performance remain unqualified.', 'Same-parent and positive-radius paths retain separate adapters and are covered here by regressions. Historical closed-source qualification remains separate. The private selection-menu screenshot records capture failure only.'],
        cases=[cases[k] for k in sorted(cases)], captures=captures,
        checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(paths)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--implementation-revision',required=True); args = parser.parse_args()
    (ROOT/'docs/fillet-open-zero-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')


if __name__ == '__main__': main()
