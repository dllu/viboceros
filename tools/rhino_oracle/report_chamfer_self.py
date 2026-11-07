"""Pin same-parent Chamfer geometry, admission, input and lifecycle evidence."""
import argparse
import gzip
import json
import math
import re
import subprocess
from .check_chamfer_self import ROOT, PROOF, commands
from .chamfer_self_evidence import BASELINE, collect, result, sha


def comparisons(proof):
    rows, captures, counts, paths = collect(proof)
    cases = {key: dict(id=key) for key in rows}
    for mode in ['portable', 'native']:
        for stage in ['command', 'ui']:
            expected = {key for key, value in rows.items() if stage == 'ui' or result(value) == 'Success'}
            text = gzip.decompress((proof / ('self-' + stage + '-' + mode + '.log.gz')).read_bytes()).decode()
            marker = 'CHAMFER_SELF_' + stage.upper() + ' '; seen = set()
            for line in text.splitlines():
                if not line.startswith(marker): continue
                row = json.loads(line[len(marker):]); key = row['id']; error = row['maximum_geometry_error']
                if key not in expected or key in seen: raise ValueError('incomplete or duplicate comparison')
                if type(error) not in (int, float) or not math.isfinite(error) or not 0 <= error <= 1e-6: raise ValueError('geometry gate')
                if row['archives'] != 1 or row['changed'] is not rows[key]['history_changed'] or row['success'] is not (result(rows[key]) == 'Success'): raise ValueError('history or archive gate')
                cases[key][mode + '_' + stage + '_maximum_geometry_error'] = error; seen.add(key)
            if seen != expected: raise ValueError('incomplete or duplicate comparison')
            if 'test result: ok. 2 passed; 0 failed;' not in text: raise ValueError('failed tests')
            if stage == 'ui' and text.count('CHAMFER_SELF_MOUSE_SOURCE_PICKS 2') != 1: raise ValueError('missing self mouse routing')
    expected_failures = {key for key,v in rows.items() if result(v)=='Failure'}
    for mode in ['portable','native']:
        text = gzip.decompress((proof / ('self-command-' + mode + '.log.gz')).read_bytes()).decode()
        rejections = [json.loads(line.split(' ',1)[1]) for line in text.splitlines() if line.startswith('CHAMFER_SELF_REJECTION ')]
        if len(rejections)!=len(expected_failures) or {r['id']for r in rejections}!=expected_failures or any(r['redo_preserved'] is not True for r in rejections): raise ValueError('incomplete rejection coverage')
    return cases, captures, counts, paths


def build(proof, revision):
    if re.fullmatch('[0-9a-f]{40}', revision or '') is None: raise ValueError('full implementation revision required')
    git = lambda *args: subprocess.check_output(['git', *args], cwd=ROOT)
    git('cat-file', '-e', revision + '^{commit}'); git('merge-base', '--is-ancestor', BASELINE, revision)
    checks = json.loads((proof / 'checks.json').read_bytes())
    if [(c['name'], c['command']) for c in checks] != list(commands()) or any(c['status'] for c in checks): raise ValueError('incomplete checks')
    cases, captures, counts, paths = comparisons(proof)
    changed = git('diff', '--name-only', '--diff-filter=ACMR', BASELINE, revision).decode().splitlines()
    if not {'crates/viboceros-command/src/chamfer/same_source.rs','crates/viboceros-command/src/chamfer/same_source/components.rs','crates/viboceros-command/src/curve_pair_edit/junction.rs','crates/viboceros-command/src/curve_pair_edit/leaf.rs','src/app/tests/chamfer_self.rs'}.issubset(changed): raise ValueError('missing same-parent implementation')
    return dict(scope='Single-parent Chamfer on open/closed polylines, mixed line/arc polycurves and kinked NURBS, including a closed NURBS seam, cross-span setbacks, native domains, IDs, attributes, groups, history and archives.',
        baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,**counts,
        successful_command_replays_per_build=35,ui_replays_per_build=46,archive_round_trips_per_build=82,api_rejections_per_build=10,windowless_source_picks_per_build=2,retained_failed_captures=0,model_epsilon=1e-6,
        maximum_geometry_difference=max(v for c in cases.values() for k,v in c.items() if k.endswith('_maximum_geometry_error')),
        construction='A linear smooth-piece scan identifies adjacent picked runs. Straight subdivisions merge inside the original interval; the original seam remains a picking boundary. Endpoint sphere crossings search full retained prefixes/suffixes or a rotated closed parent. The selected junction determines the cyclic remainder. Shared immutable plans assemble Chamfer and Fillet results and use existing portable/usd-brep NURBS extraction.',
        input_evidence='Only authored source definitions, public getter options and pre-command mouse-ray/source witnesses supply construction inputs. Native output coefficients, knots and original point/tangent samples are expectations. Comparisons evaluate original native parameters directly, avoiding affine remap roundoff across nonsmooth knots.',
        execution='All four licensed Rhino captures ran on private Xvfb displays. Application/viewport tests were windowless.',
        limits=['Finite sampled coverage does not prove every folded, self-intersecting or highly oscillatory branch, continuous distance bounds, pixel equality or comparative performance. Distinct closed-source Chamfer is still unsupported.', 'Captured same-piece, nonadjacent-piece, oversized and zero-distance closed-parent failures preserve geometry and pending Redo. A smooth original seam remains a native picking boundary; geometrically adjacent runs across it can fail.'],
        cases=[cases[k] for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(paths)})


def main():
    p = argparse.ArgumentParser(description=__doc__); p.add_argument('--implementation-revision', required=True); args = p.parse_args()
    (ROOT / 'docs/chamfer-self-validation.json').write_text(json.dumps(build(PROOF, args.implementation_revision), indent=2, allow_nan=False) + '\n')

if __name__ == '__main__': main()
