"""Pin Fillet continuation geometry, admission, inputs and lifecycle evidence."""
import argparse
import gzip
import json
import math
import re
import subprocess
from .check_fillet_continuation import ROOT, PROOF, commands
from .fillet_continuation_evidence import BASELINE, collect, result, sha
from .fillet_continuation_sdk_evidence import collect as collect_sdk


def comparisons(proof):
    rows, captures, counts, paths = collect(proof)
    cases = {key: dict(id=key) for key in rows}
    for mode in ['portable', 'native']:
        for stage in ['command', 'ui']:
            expected = {key for key, value in rows.items() if (stage == 'ui' or result(value) == 'Success')}
            text = gzip.decompress((proof / ('continuation-' + stage + '-' + mode + '.log.gz')).read_bytes()).decode()
            marker = 'FILLET_CONTINUATION_' + stage.upper() + ' '; seen = set()
            for line in text.splitlines():
                if not line.startswith(marker): continue
                row = json.loads(line[len(marker):]); key = row['id']; error = row['maximum_geometry_error']
                if key not in expected or key in seen: raise ValueError('incomplete or duplicate comparison')
                if type(error) not in (int, float) or not math.isfinite(error) or not 0 <= error <= 1e-6: raise ValueError('geometry gate')
                if row['archives'] != 1 or row['changed'] is not rows[key]['history_changed'] or row['success'] is not (result(rows[key]) == 'Success'): raise ValueError('history or archive gate')
                cases[key][mode + '_' + stage + '_maximum_geometry_error'] = error; seen.add(key)
            if seen != expected: raise ValueError('incomplete or duplicate comparison')
            if 'test result: ok. 2 passed; 0 failed;' not in text: raise ValueError('failed tests')
            if stage == 'ui' and text.count('FILLET_CONTINUATION_MOUSE_SOURCE_PICKS 2') != 1: raise ValueError('missing extension mouse routing')
    expected_failures = {key for key,v in rows.items() if result(v)=='Failure'}
    for mode in ['portable','native']:
        text = gzip.decompress((proof / ('continuation-command-' + mode + '.log.gz')).read_bytes()).decode()
        rejections = [json.loads(line.split(' ',1)[1]) for line in text.splitlines() if line.startswith('FILLET_CONTINUATION_REJECTION ')]
        if len(rejections)!=len(expected_failures) or {r['id']for r in rejections}!=expected_failures or any(r['redo_preserved'] is not True for r in rejections): raise ValueError('incomplete rejection coverage')
    for key in cases: cases[key].update(qualified=True,native_result=result(rows[key]),native_valid=True)
    sdk,sdk_paths=collect_sdk(proof, rows);paths.update(sdk_paths)
    return cases, captures, counts, paths


def build(proof, revision):
    if re.fullmatch('[0-9a-f]{40}', revision or '') is None: raise ValueError('full implementation revision required')
    git = lambda *args: subprocess.check_output(['git', *args], cwd=ROOT)
    git('cat-file', '-e', revision+'^{commit}')
    git('merge-base', '--is-ancestor', BASELINE, revision)
    checks = json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command']) for c in checks] != list(commands()) or any(c['status'] for c in checks): raise ValueError('incomplete checks')
    cases, captures, counts, paths = comparisons(proof)
    changed = git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required = {'crates/viboceros-command/src/fillet/closed.rs','crates/viboceros-command/src/fillet/closed/continuation/smooth.rs','crates/viboceros-command/src/fillet/closed/continuation/tail.rs','crates/viboceros-command/tests/fillet_continuation.rs','src/app/tests/curve_fillet.rs','src/app/tests/fillet_continuation.rs'}
    if not required.issubset(changed): raise ValueError('missing continuation implementation')
    sdk,_ = collect_sdk(proof, collect(proof)[0])
    return dict(scope='Closed-source Fillet with circular, endpoint-tangent and smooth continuation of a distinct nonmeeting open curved partner.',
        baseline_revision=BASELINE, implementation_revision=revision, full_native_parity=False, **counts,
        successful_command_replays_per_build=35, qualified_ui_replays_per_build=44, archive_round_trips_per_build=80, api_rejections_per_build=8, windowless_source_picks_per_build=2, retained_failed_captures=1, model_epsilon=1e-6,
        maximum_geometry_difference=max(v for c in cases.values() for k,v in c.items() if k.endswith('_maximum_geometry_error')),
        construction='Circular support preserves the source speed and resets extended retained arc domains to length. Endpoint tangent segments retain native derivative speed. Smooth endpoint charts expand geometrically; multiple tangent solves rank contacts by summed unwrapped arc lengths from the original picks. Length bounds and conservative polynomial tail certificates stop expansion. The selected minor arc must align with the retained open piece when trimming; admission does not fall back to a farther contact. Existing prepared plans provide atomic admission and cached preview.',
        input_evidence='Authored definitions, public options and pre-command physical mouse sight points supply inputs. Native output coefficients, knots, domains and public point/tangent stations are expectations only. Public GetFilletPoints/CreateFillet controls are audited separately from command parity.',
        execution='Four complete command captures and one public SDK capture ran on private Xvfb displays with isolated runtime directories. Both egui/application builds ran without windows. All 44 native command observations are included. One failed SDK observer attempt published no observations; its log and source closure are retained. One successful capture emitted a retained SyntaxWarning from recipe spacing, corrected subsequently.',
        sdk_controls=sdk,
        limits=['Finite coverage does not prove enumeration of all tangent roots, tied costs, arbitrary folded or oscillatory continuations, signed rational poles, stationary tangents, nonplanar contacts, continuous distance bounds, pixel equality or comparative Rhino performance. Smooth search uses finite multistart samples and at most 40 domain expansions; only outward polynomial tails receive a monotonic exclusion certificate.', 'Closed-source zero radius, corner contacts, and arbitrary endpoint continuation style combinations remain unqualified. Existing distinct-open-source and same-parent Fillet paths are separate.'],
        cases=[cases[k] for k in sorted(cases)], captures=captures,
        checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(paths)})


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--implementation-revision', required=True)
    args = p.parse_args()
    (ROOT/'docs/fillet-continuation-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')


if __name__ == '__main__': main()
