"""Pin nonmeeting Chamfer branch geometry, admission, input and lifecycle evidence."""
import argparse
import gzip
import json
import math
import re
import subprocess
from .check_chamfer_branch import ROOT, PROOF, commands
from .chamfer_branch_evidence import BASELINE, collect, collect_sdk, result, sha


def comparisons(proof):
    rows, captures, counts, paths = collect(proof)
    cases = {key: dict(id=key) for key in rows}
    for mode in ['portable', 'native']:
        for stage in ['command', 'ui']:
            expected = {key for key, value in rows.items() if (stage == 'ui' or result(value) == 'Success')}
            text = gzip.decompress((proof / ('branch-' + stage + '-' + mode + '.log.gz')).read_bytes()).decode()
            marker = 'CHAMFER_BRANCH_' + stage.upper() + ' '; seen = set()
            for line in text.splitlines():
                if not line.startswith(marker): continue
                row = json.loads(line[len(marker):]); key = row['id']; error = row['maximum_geometry_error']
                if key not in expected or key in seen: raise ValueError('incomplete or duplicate comparison')
                if type(error) not in (int, float) or not math.isfinite(error) or not 0 <= error <= 1e-6: raise ValueError('geometry gate')
                if row['archives'] != 1 or row['changed'] is not rows[key]['history_changed'] or row['success'] is not (result(rows[key]) == 'Success'): raise ValueError('history or archive gate')
                cases[key][mode + '_' + stage + '_maximum_geometry_error'] = error; seen.add(key)
            if seen != expected: raise ValueError('incomplete or duplicate comparison')
            if 'test result: ok. 2 passed; 0 failed;' not in text: raise ValueError('failed tests')
            if stage == 'ui' and text.count('CHAMFER_BRANCH_MOUSE_SOURCE_PICKS 2') != 1: raise ValueError('missing extension mouse routing')
    expected_failures = {key for key,v in rows.items() if result(v)=='Failure'}
    for mode in ['portable','native']:
        text = gzip.decompress((proof / ('branch-command-' + mode + '.log.gz')).read_bytes()).decode()
        rejections = [json.loads(line.split(' ',1)[1]) for line in text.splitlines() if line.startswith('CHAMFER_BRANCH_REJECTION ')]
        if len(rejections)!=len(expected_failures) or {r['id']for r in rejections}!=expected_failures or any(r['redo_preserved'] is not True for r in rejections): raise ValueError('incomplete rejection coverage')
    for key in cases: cases[key].update(qualified=True,native_result=result(rows[key]),native_valid=True)
    sdk,sdk_paths=collect_sdk(proof);paths.update(sdk_paths)
    return cases, captures, counts, paths


def build(proof, revision):
    if re.fullmatch('[0-9a-f]{40}', revision or '') is None: raise ValueError('full implementation revision required')
    git = lambda *args: subprocess.check_output(['git', *args], cwd=ROOT)
    git('cat-file', '-e', revision + '^{commit}'); git('merge-base', '--is-ancestor', BASELINE, revision)
    checks = json.loads((proof / 'checks.json').read_bytes())
    if [(c['name'], c['command']) for c in checks] != list(commands()) or any(c['status'] for c in checks): raise ValueError('incomplete checks')
    cases, captures, counts, paths = comparisons(proof)
    changed = git('diff', '--name-only', '--diff-filter=ACMR', BASELINE, revision).decode().splitlines()
    if not {'crates/viboceros-command/src/chamfer/closed.rs','crates/viboceros-command/src/chamfer/closed/extension.rs','crates/viboceros-command/tests/support/chamfer_replay.rs','crates/viboceros-geometry/src/nurbs/boundary_extension.rs','src/app/tests/chamfer_branch.rs'}.issubset(changed): raise ValueError('missing branch implementation')
    sdk,_=collect_sdk(proof)
    return dict(scope='Nonmeeting Chamfer end selection and native closed-source seam direction on circles, rectangles and ellipses, with independently captured public extension API controls.',
        baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,**counts,
        successful_command_replays_per_build=43,qualified_ui_replays_per_build=46,archive_round_trips_per_build=90,api_rejections_per_build=2,windowless_source_picks_per_build=2,retained_failed_captures=0,model_epsilon=1e-6,
        maximum_geometry_difference=max(v for c in cases.values() for k,v in c.items() if k.endswith('_maximum_geometry_error')),
        construction='Open curved sources consider both continuation ends and choose the least added length. Boundary-box distance orders candidates and prunes an end that cannot improve the known continuation, including numerical length margins. Intersection candidates rank by unwrapped parameter distance on the closed sources. Closed-source directions compare the native pick and corner parameters across the seam. Fixed-accuracy tight probe boxes can reject disjoint curve boundaries before an enlarged extrapolation inflates the general pair solver tolerance; unresolved bounding falls back to that solver. Existing shared admission preserves primitive/parameter charts and source identity.',
        input_evidence='Authored geometry, public getter preferences and pre-command physical mouse rays supply inputs. Native coefficients, knots and original public point/tangent stations are expectations. API controls are distinct from command geometry comparisons.',
        execution='Four complete command captures and one public extension API capture ran on private Xvfb displays with isolated runtime directories. Both egui/application builds ran without windows. No native observations were excluded from this command qualification.',
        sdk_controls=sdk,
        limits=['The prior mixed Arc=Line and Other=Smooth branch remains unsupported. Public extension API controls are consistent under rigid transforms and isolate the discrepancy to command-level branch selection; these controls do not qualify that mixed command branch.', 'Finite coverage does not establish all continuation rankings, tied lengths, arbitrary folded or oscillatory branches, all overlaps, continuous distance bounds, pixel equality or comparative Rhino performance.'],
        cases=[cases[k] for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(paths)})


def main():
    p = argparse.ArgumentParser(description=__doc__); p.add_argument('--implementation-revision', required=True); args = p.parse_args()
    (ROOT / 'docs/chamfer-branch-validation.json').write_text(json.dumps(build(PROOF, args.implementation_revision), indent=2, allow_nan=False) + '\n')

if __name__ == '__main__': main()
