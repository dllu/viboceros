"""Pin distinct closed-source Chamfer geometry, admission, input and lifecycle evidence."""
import argparse
import gzip
import json
import math
import re
import subprocess
from .check_chamfer_closed import ROOT, PROOF, commands
from .chamfer_closed_evidence import INVALID, BASELINE, collect, result, sha


def comparisons(proof):
    rows, captures, counts, paths = collect(proof)
    cases = {key: dict(id=key) for key in rows}
    for mode in ['portable', 'native']:
        for stage in ['command', 'ui']:
            expected = {key for key, value in rows.items() if key not in INVALID and (stage == 'ui' or result(value) == 'Success')}
            text = gzip.decompress((proof / ('closed-' + stage + '-' + mode + '.log.gz')).read_bytes()).decode()
            marker = 'CHAMFER_CLOSED_' + stage.upper() + ' '; seen = set()
            for line in text.splitlines():
                if not line.startswith(marker): continue
                row = json.loads(line[len(marker):]); key = row['id']; error = row['maximum_geometry_error']
                if key not in expected or key in seen: raise ValueError('incomplete or duplicate comparison')
                if type(error) not in (int, float) or not math.isfinite(error) or not 0 <= error <= 1e-6: raise ValueError('geometry gate')
                if row['archives'] != 1 or row['changed'] is not rows[key]['history_changed'] or row['success'] is not (result(rows[key]) == 'Success'): raise ValueError('history or archive gate')
                cases[key][mode + '_' + stage + '_maximum_geometry_error'] = error; seen.add(key)
            if seen != expected: raise ValueError('incomplete or duplicate comparison')
            if 'test result: ok. 2 passed; 0 failed;' not in text: raise ValueError('failed tests')
            if stage == 'ui' and text.count('CHAMFER_CLOSED_MOUSE_SOURCE_PICKS 2') != 1: raise ValueError('missing self mouse routing')
    expected_failures = {key for key,v in rows.items() if result(v)=='Failure'} | INVALID
    for mode in ['portable','native']:
        text = gzip.decompress((proof / ('closed-command-' + mode + '.log.gz')).read_bytes()).decode()
        rejections = [json.loads(line.split(' ',1)[1]) for line in text.splitlines() if line.startswith('CHAMFER_CLOSED_REJECTION ')]
        if len(rejections)!=len(expected_failures) or {r['id']for r in rejections}!=expected_failures or any(r['redo_preserved'] is not True for r in rejections): raise ValueError('incomplete rejection coverage')
    for mode in ['portable','native']:
        text=gzip.decompress((proof/('closed-ui-'+mode+'.log.gz')).read_bytes()).decode()
        rejected=[json.loads(line.split(' ',1)[1]) for line in text.splitlines() if line.startswith('CHAMFER_CLOSED_UI_REJECTION ')]
        if len(rejected)!=2 or {r['id'] for r in rejected}!=INVALID or any(r['redo_preserved'] is not True for r in rejected): raise ValueError('invalid native guard coverage')
    for key in cases: cases[key].update(qualified=key not in INVALID,native_invalid=key in INVALID)
    return cases, captures, counts, paths


def build(proof, revision):
    if re.fullmatch('[0-9a-f]{40}', revision or '') is None: raise ValueError('full implementation revision required')
    git = lambda *args: subprocess.check_output(['git', *args], cwd=ROOT)
    git('cat-file', '-e', revision + '^{commit}'); git('merge-base', '--is-ancestor', BASELINE, revision)
    checks = json.loads((proof / 'checks.json').read_bytes())
    if [(c['name'], c['command']) for c in checks] != list(commands()) or any(c['status'] for c in checks): raise ValueError('incomplete checks')
    cases, captures, counts, paths = comparisons(proof)
    changed = git('diff', '--name-only', '--diff-filter=ACMR', BASELINE, revision).decode().splitlines()
    if not {'crates/viboceros-command/src/chamfer/closed.rs','crates/viboceros-command/src/chamfer/closed/support.rs','crates/viboceros-command/src/curve_pair_edit/closed.rs','src/app/tests/chamfer_closed.rs','tools/rhino_oracle/line_scene_probe.py','tools/rhino_oracle/conic_getter_input.py'}.issubset(changed): raise ValueError('missing closed-source implementation')
    return dict(scope='Distinct closed-source Chamfer on circles, closed polylines and rational NURBS, including ellipse and nonlinear open-NURBS contacts, tangent and coincident intersections, line extension, native metadata, IDs and history.',
        baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,**counts,
        successful_command_replays_per_build=28,qualified_ui_replays_per_build=31,archive_round_trips_per_build=60,api_rejections_per_build=4,windowless_source_picks_per_build=2,retained_failed_captures=3,guarded_invalid_native_successes_per_build=2,model_epsilon=1e-6,
        maximum_geometry_difference=max(v for c in cases.values() for k,v in c.items() if k.endswith('_maximum_geometry_error')),
        construction='Portable/usd-brep curve intersections supply isolated points and overlap boundaries. Picks rank the intersection and choose each local direction. Infinite line supports are bounded against the other source, preserving original charts. Endpoint sphere crossings find each Euclidean setback. A shared contact-admission plan preserves closed parents and trims only open sources.',
        input_evidence='Authored sources, public getter options and pre-command mouse rays supply construction inputs. Coincident curves use two public transparent SelID commands to avoid ambiguous physical hits, with authored natural-end seed points. Output coefficients, original point/tangent stations and invalid native diagnostics are expectations.',
        execution='Three successful captures and three retained unsuccessful attempts ran on private Xvfb displays. No observations were published by unsuccessful attempts. Perspective selection ambiguity was repeated in Top view with the same geometry/picks. An invalid native result required a validity-aware observer; typed SelID required correcting the first/second curve prompt gate. Application and viewport checks were windowless.',
        limits=['Two captured native successes create invalid polycurves: zero distances with two closed sources and Join, and a joined cut consuming the open line. Viboceros rejects both without edits or Redo loss; they are excluded from geometry parity.', 'Finite sampled coverage does not qualify arbitrary folded or oscillatory branches, all overlaps, continuous distance bounds, pixel equality or comparative performance. Nonmeeting open arc/NURBS extensions against a closed source still need broader qualification.'],
        cases=[cases[k] for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(paths)})


def main():
    p = argparse.ArgumentParser(description=__doc__); p.add_argument('--implementation-revision', required=True); args = p.parse_args()
    (ROOT / 'docs/chamfer-closed-validation.json').write_text(json.dumps(build(PROOF, args.implementation_revision), indent=2, allow_nan=False) + '\n')

if __name__ == '__main__': main()
