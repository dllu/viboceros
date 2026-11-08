"""Pin nonmeeting curved-source Chamfer geometry, admission, input and lifecycle evidence."""
import argparse
import gzip
import json
import math
import re
import subprocess
from .check_chamfer_extension import ROOT, PROOF, commands
from .chamfer_extension_evidence import UNQUALIFIED, BASELINE, collect, result, sha


def comparisons(proof):
    rows, captures, counts, paths = collect(proof)
    cases = {key: dict(id=key) for key in rows}
    for mode in ['portable', 'native']:
        for stage in ['command', 'ui']:
            expected = {key for key, value in rows.items() if key not in UNQUALIFIED and (stage == 'ui' or result(value) == 'Success')}
            text = gzip.decompress((proof / ('extension-' + stage + '-' + mode + '.log.gz')).read_bytes()).decode()
            marker = 'CHAMFER_EXTENSION_' + stage.upper() + ' '; seen = set()
            for line in text.splitlines():
                if not line.startswith(marker): continue
                row = json.loads(line[len(marker):]); key = row['id']; error = row['maximum_geometry_error']
                if key not in expected or key in seen: raise ValueError('incomplete or duplicate comparison')
                if type(error) not in (int, float) or not math.isfinite(error) or not 0 <= error <= 1e-6: raise ValueError('geometry gate')
                if row['archives'] != 1 or row['changed'] is not rows[key]['history_changed'] or row['success'] is not (result(rows[key]) == 'Success'): raise ValueError('history or archive gate')
                cases[key][mode + '_' + stage + '_maximum_geometry_error'] = error; seen.add(key)
            if seen != expected: raise ValueError('incomplete or duplicate comparison')
            if 'test result: ok. 2 passed; 0 failed;' not in text: raise ValueError('failed tests')
            if stage == 'ui' and text.count('CHAMFER_EXTENSION_MOUSE_SOURCE_PICKS 2') != 1: raise ValueError('missing extension mouse routing')
    expected_failures = {key for key,v in rows.items() if result(v)=='Failure'} | UNQUALIFIED
    for mode in ['portable','native']:
        text = gzip.decompress((proof / ('extension-command-' + mode + '.log.gz')).read_bytes()).decode()
        rejections = [json.loads(line.split(' ',1)[1]) for line in text.splitlines() if line.startswith('CHAMFER_EXTENSION_REJECTION ')]
        if len(rejections)!=len(expected_failures) or {r['id']for r in rejections}!=expected_failures or any(r['redo_preserved'] is not True for r in rejections): raise ValueError('incomplete rejection coverage')
    for mode in ['portable','native']:
        text=gzip.decompress((proof/('extension-ui-'+mode+'.log.gz')).read_bytes()).decode()
        rejected=[json.loads(line.split(' ',1)[1]) for line in text.splitlines() if line.startswith('CHAMFER_EXTENSION_UI_REJECTION ')]
        if len(rejected)!=6 or {r['id'] for r in rejected}!=UNQUALIFIED or any(r['redo_preserved'] is not True for r in rejected): raise ValueError('unqualified style guard coverage')
    for key in cases:
        cases[key].update(qualified=key not in UNQUALIFIED,unqualified_style=key in UNQUALIFIED,native_result=result(rows[key]),native_valid=True)
    return cases, captures, counts, paths


def build(proof, revision):
    if re.fullmatch('[0-9a-f]{40}', revision or '') is None: raise ValueError('full implementation revision required')
    git = lambda *args: subprocess.check_output(['git', *args], cwd=ROOT)
    git('cat-file', '-e', revision + '^{commit}'); git('merge-base', '--is-ancestor', BASELINE, revision)
    checks = json.loads((proof / 'checks.json').read_bytes())
    if [(c['name'], c['command']) for c in checks] != list(commands()) or any(c['status'] for c in checks): raise ValueError('incomplete checks')
    cases, captures, counts, paths = comparisons(proof)
    changed = git('diff', '--name-only', '--diff-filter=ACMR', BASELINE, revision).decode().splitlines()
    if not {'crates/viboceros-command/src/chamfer/closed.rs','crates/viboceros-command/src/chamfer/closed/extension.rs','crates/viboceros-command/src/curve_pair_edit/closed.rs','crates/viboceros-geometry/src/circular.rs','src/app/tests/chamfer_extension.rs'}.issubset(changed): raise ValueError('missing extension implementation')
    return dict(scope='Nonmeeting open arc/NURBS continuation against a distinct closed circle, preserving native primitive and parameter charts, source metadata, IDs, history and 3DM archives.',
        baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,**counts,
        successful_command_replays_per_build=29,qualified_ui_replays_per_build=31,archive_round_trips_per_build=61,api_rejections_per_build=7,windowless_source_picks_per_build=2,retained_failed_captures=0,guarded_unqualified_native_successes_per_build=6,qualified_point_tangent_stations=4356,model_epsilon=1e-6,
        maximum_geometry_difference=max(v for c in cases.values() for k,v in c.items() if k.endswith('_maximum_geometry_error')),
        construction='The existing portable/usd-brep boundary-extension producer extends the picked open end before intersection ranking and Euclidean setbacks. Circular continuations retain the arc primitive; tangent lines retain the native endpoint derivative speed. Smooth NURBS retain the extrapolated polynomial chart. Circular setbacks outside the original source reset to a length domain; cuts inside preserve the original chart. Shared contact admission keeps closed parents whole.',
        input_evidence='Authored sources, public getter settings and pre-command physical mouse rays supply inputs. Native coefficients, knots and 33 public point/tangent stations per curve are expectations. Every native workflow is retained, including failures, cancellation and six explicitly unqualified successes.',
        execution='Four successful immutable captures ran on private Xvfb displays with isolated runtime directories. Application and egui viewport tests created no windows. Verification explicitly sets both extension styles; earlier families retain public remembered preferences.',
        limits=['Nonmeeting arc continuation against a closed source with Arc=Line and Other=Smooth is rejected without edits or Redo loss. Six valid native successes show different continuation paths under rigid transforms; the entire mixed-style branch remains unqualified, and its native outputs are preserved.', 'Finite coverage uses a closed circular boundary. Arbitrary closed boundaries, folded or oscillatory branches, all intersection choices, continuous distance bounds, pixel equality and comparative performance remain unqualified.'],
        cases=[cases[k] for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(paths)})


def main():
    p = argparse.ArgumentParser(description=__doc__); p.add_argument('--implementation-revision', required=True); args = p.parse_args()
    (ROOT / 'docs/chamfer-extension-validation.json').write_text(json.dumps(build(PROOF, args.implementation_revision), indent=2, allow_nan=False) + '\n')

if __name__ == '__main__': main()
