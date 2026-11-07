"""Pin closed-source Fillet geometry, admission, input and lifecycle evidence."""
import argparse
import gzip
import json
import math
import re
import subprocess
from .check_fillet_closed import ROOT, PROOF, commands
from .fillet_closed_evidence import BASELINE, collect, result, sha


def comparisons(proof):
    rows, captures, counts, paths = collect(proof)
    cases = {key: dict(id=key) for key in rows}
    for mode in ['portable', 'native']:
        for stage in ['command', 'ui']:
            expected = {key for key, value in rows.items() if stage == 'ui' or result(value) == 'Success'}
            text = gzip.decompress((proof / ('closed-' + stage + '-' + mode + '.log.gz')).read_bytes()).decode()
            marker = 'FILLET_CLOSED_' + stage.upper() + ' '; seen = set()
            for line in text.splitlines():
                if not line.startswith(marker): continue
                row = json.loads(line[len(marker):]); key = row['id']; error = row['maximum_geometry_error']
                if key not in expected or key in seen: raise ValueError('incomplete or duplicate comparison')
                if type(error) not in (int, float) or not math.isfinite(error) or not 0 <= error <= 1e-6: raise ValueError('geometry gate')
                if row['archives'] != 1 or row['changed'] is not rows[key]['history_changed'] or row['success'] is not (result(rows[key]) == 'Success'): raise ValueError('history or archive gate')
                cases[key][mode + '_' + stage + '_maximum_geometry_error'] = error; seen.add(key)
            if seen != expected: raise ValueError('incomplete or duplicate comparison')
            if ('test result: ok. 1 passed; 0 failed;' if stage == 'command' else 'test result: ok. 2 passed; 0 failed;') not in text: raise ValueError('failed tests')
            if stage == 'ui' and text.count('FILLET_CLOSED_MOUSE_SOURCE_PICKS 2') != 1: raise ValueError('missing closed mouse routing')
    return cases, captures, counts, paths


def build(proof, revision):
    if re.fullmatch('[0-9a-f]{40}', revision or '') is None: raise ValueError('full implementation revision required')
    git = lambda *args: subprocess.check_output(['git', *args], cwd=ROOT)
    git('cat-file', '-e', revision + '^{commit}'); git('merge-base', '--is-ancestor', BASELINE, revision)
    checks = json.loads((proof / 'checks.json').read_bytes())
    if [(c['name'], c['command']) for c in checks] != list(commands()) or any(c['status'] for c in checks): raise ValueError('incomplete checks')
    cases, captures, counts, paths = comparisons(proof)
    changed = git('diff', '--name-only', '--diff-filter=ACMR', BASELINE, revision).decode().splitlines()
    if not {'crates/viboceros-command/src/fillet/closed.rs', 'crates/viboceros-command/src/fillet/prepare.rs', 'crates/viboceros-command/tests/fillet_closed.rs', 'src/app/tests/fillet_closed.rs'}.issubset(changed): raise ValueError('missing closed implementation')
    return dict(scope='Positive-radius Fillet with a distinct planar closed source: preserve closed parents, trim the picked side of open sources, apply ordered join identity and metadata, dynamic preview/cancel, history and 3DM archives.',
                baseline_revision=BASELINE, implementation_revision=revision, full_native_parity=False,
                **counts, successful_command_replays_per_build=15, ui_replays_per_build=16, archive_round_trips_per_build=31,
                windowless_source_picks_per_build=2, retained_failed_captures=2, model_epsilon=1e-6,
                maximum_geometry_difference=max(v for c in cases.values() for k, v in c.items() if k.endswith('_maximum_geometry_error')),
                construction='Existing tangent-circle solver uses source parameters seeded from picks. Closed parents stay unchanged. Retained open NURBS intervals use the selected portable/usd-brep extractor; lines retain their parameter charts. One prepared edit plan supplies previews and atomic commits. Native outputs are expectations only.',
                input_evidence='Prescribed source definitions, settings, acknowledged private mouse rays and pre-command public curve/sight-line witnesses construct replays. Construction-plane projection is insufficient for tilted sources; the spatial holdout uses the recorded sight-line witness.',
                execution='Native captures used private Xvfb displays; application and viewport tests were windowless. The failed perspective capture was terminated after verifying its owned PID and settings scheme; no desktop input was sent.',
                limits=['Coverage includes circles, a rational NURBS circle, an ellipse, internal/external and upper/lower branches, reversed source order, tilted spatial sources and dynamic cancellation. This is a finite corpus, not every branch or a continuous distance bound.', 'Self-source fillets, zero radius with closed sources, nonplanar closed curves, closed polylines with corner contacts and extension of open curved sources remain unqualified. No pixel equality or comparative performance claim.'],
                cases=[cases[k] for k in sorted(cases)], captures=captures,
                checks=[dict(name=c['name'], status=c['status']) for c in checks],
                implementation_git_sha256={revision + ':' + p: sha(git('cat-file', 'blob', revision + ':' + p)) for p in changed},
                evidence_sha256={str(p.relative_to(ROOT)): sha(p.read_bytes()) for p in sorted(paths)})


def main():
    p = argparse.ArgumentParser(description=__doc__); p.add_argument('--implementation-revision', required=True); args = p.parse_args()
    (ROOT / 'docs/fillet-closed-validation.json').write_text(json.dumps(build(PROOF, args.implementation_revision), indent=2, allow_nan=False) + '\n')

if __name__ == '__main__': main()
