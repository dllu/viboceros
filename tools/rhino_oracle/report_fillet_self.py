"""Pin same-parent Fillet geometry, admission, input and lifecycle evidence."""
import argparse
import gzip
import json
import math
import re
import subprocess
from .check_fillet_self import ROOT, PROOF, commands
from .fillet_self_evidence import BASELINE, collect, result, sha


def comparisons(proof):
    rows, captures, counts, paths = collect(proof)
    cases = {key: dict(id=key) for key in rows}
    for mode in ['portable', 'native']:
        for stage in ['command', 'ui']:
            expected = {key for key, value in rows.items() if stage == 'ui' or result(value) == 'Success'}
            text = gzip.decompress((proof / ('self-' + stage + '-' + mode + '.log.gz')).read_bytes()).decode()
            marker = 'FILLET_SELF_' + stage.upper() + ' '; seen = set()
            for line in text.splitlines():
                if not line.startswith(marker): continue
                row = json.loads(line[len(marker):]); key = row['id']; error = row['maximum_geometry_error']
                if key not in expected or key in seen: raise ValueError('incomplete or duplicate comparison')
                if type(error) not in (int, float) or not math.isfinite(error) or not 0 <= error <= 1e-6: raise ValueError('geometry gate')
                if row['archives'] != 1 or row['changed'] is not rows[key]['history_changed'] or row['success'] is not (result(rows[key]) == 'Success'): raise ValueError('history or archive gate')
                cases[key][mode + '_' + stage + '_maximum_geometry_error'] = error; seen.add(key)
            if seen != expected: raise ValueError('incomplete or duplicate comparison')
            if 'test result: ok. 2 passed; 0 failed;' not in text: raise ValueError('failed tests')
            if stage == 'ui' and text.count('FILLET_SELF_MOUSE_SOURCE_PICKS 2') != 1: raise ValueError('missing self mouse routing')
    expected_failures = {key for key,v in rows.items() if result(v)=='Failure'}
    for mode in ['portable','native']:
        text = gzip.decompress((proof / ('self-command-' + mode + '.log.gz')).read_bytes()).decode()
        rejections = [json.loads(line.split(' ',1)[1]) for line in text.splitlines() if line.startswith('FILLET_SELF_REJECTION ')]
        if len(rejections)!=len(expected_failures) or {r['id']for r in rejections}!=expected_failures or any(r['redo_preserved'] is not True for r in rejections): raise ValueError('incomplete rejection coverage')
    geometry = gzip.decompress((proof / 'geometry-decompose.log.gz').read_bytes()).decode()
    if any(name not in geometry for name in ['c0_batch_keeps_closed_curve_domain_order_and_exact_weight_gauges','c0_batch_splits_shared_and_independent_controls_without_changing_evaluation','test result: ok. 7 passed; 0 failed;']): raise ValueError('missing batch decomposition coverage')
    return cases, captures, counts, paths


def build(proof, revision):
    if re.fullmatch('[0-9a-f]{40}', revision or '') is None: raise ValueError('full implementation revision required')
    git = lambda *args: subprocess.check_output(['git', *args], cwd=ROOT)
    git('cat-file', '-e', revision + '^{commit}'); git('merge-base', '--is-ancestor', BASELINE, revision)
    checks = json.loads((proof / 'checks.json').read_bytes())
    if [(c['name'], c['command']) for c in checks] != list(commands()) or any(c['status'] for c in checks): raise ValueError('incomplete checks')
    cases, captures, counts, paths = comparisons(proof)
    changed = git('diff', '--name-only', '--diff-filter=ACMR', BASELINE, revision).decode().splitlines()
    if not {'crates/viboceros-command/src/fillet/same_source.rs','crates/viboceros-command/src/fillet/same_source/select.rs','crates/viboceros-geometry/src/nurbs/decompose.rs','src/app/curve_fillet.rs','src/app/tests/fillet_self.rs'}.issubset(changed): raise ValueError('missing same-parent implementation')
    return dict(scope='Same-parent Fillet on open/closed polylines, mixed polycurves and kinked NURBS, including a closed NURBS seam, native domains, source identity, split metadata, preview/cancel, no-ops and rejections.',
                baseline_revision=BASELINE, implementation_revision=revision, full_native_parity=False,
                **counts, successful_command_replays_per_build=30, ui_replays_per_build=38, archive_round_trips_per_build=68,
                api_rejections_per_build=5, windowless_source_picks_per_build=2, retained_failed_captures=0, model_epsilon=1e-6,
                maximum_geometry_difference=max(v for c in cases.values() for k,v in c.items() if k.endswith('_maximum_geometry_error')),
                construction='A linear scan of neighboring smooth pieces ranks fillets by distance from pick seeds in the original parameter chart. Active coincident line offsets reject a singular angle. Existing portable/usd-brep producers solve junctions and extract retained NURBS intervals. Batch C0 splitting copies output control blocks rather than repeatedly trimming the full source. Closed NURBS seam kinks are exposed by a private midpoint split. One prepared edit plan supplies previews and commits.',
                input_evidence='Only prescribed source definitions, options and public pre-command mouse-ray/source witnesses construct replays. Native output coefficients, domains, samples and pixels are expectations.',
                execution='All five licensed Rhino captures ran on private Xvfb displays. Application and viewport tests were windowless.',
                limits=['The corpus covers 28 edits, two successful no-ops, five failures and three cancellations. It includes reversed picks and sources, nonadjacent picks, folded paths, near-corner picks, far-origin and tilted geometry, dynamic option/radius transitions and parallel offset singularities. This is sampled finite coverage, not all possible branches or a continuous distance bound.', 'Fully consumed multi-span junctions, arbitrary smooth self-bitangencies and all highly folded or self-intersecting sources remain unqualified. No pixel equality or comparative performance claim. Source decomposition and candidate search still require work proportional to the number of pieces; non-C0 NURBS cuts need interval extraction.'],
                cases=[cases[k] for k in sorted(cases)], captures=captures,
                checks=[dict(name=c['name'], status=c['status']) for c in checks],
                implementation_git_sha256={revision + ':' + p: sha(git('cat-file', 'blob', revision + ':' + p)) for p in changed},
                evidence_sha256={str(p.relative_to(ROOT)): sha(p.read_bytes()) for p in sorted(paths)})


def main():
    p = argparse.ArgumentParser(description=__doc__); p.add_argument('--implementation-revision', required=True); args = p.parse_args()
    (ROOT / 'docs/fillet-self-validation.json').write_text(json.dumps(build(PROOF, args.implementation_revision), indent=2, allow_nan=False) + '\n')

if __name__ == '__main__': main()
