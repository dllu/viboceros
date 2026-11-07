"""Pin ordered Fillet geometry, dynamic preview and document lifecycle evidence."""
import argparse
import gzip
import json
import math
import re
import subprocess
from .fillet_getter_evidence import BASELINE, collect, result, sha
from .check_fillet_getter import ROOT, PROOF, commands


def comparisons(proof):
    rows, diagnostics, captures, counts, paths = collect(proof)
    contacts = gzip.decompress((proof / 'native-contacts.log.gz').read_bytes()).decode()
    if 'native_radius_frames_preserve_tangencies_at_small_and_large_model_scales' not in contacts or 'test result: ok. 6 passed; 0 failed;' not in contacts: raise ValueError('missing native arc chart coverage')
    cases = {key: dict(id=key) for key in rows}
    for mode in ['portable', 'native']:
        for stage in ['command', 'ui']:
            expected = {key for key, value in rows.items() if stage == 'ui' or result(value) == 'Success'}
            text = gzip.decompress((proof / (stage + '-' + mode + '.log.gz')).read_bytes()).decode()
            marker = 'FILLET_GETTER_' + stage.upper() + ' '
            seen = set()
            for line in text.splitlines():
                if not line.startswith(marker): continue
                row = json.loads(line[len(marker):]); key = row['id']; error = row['maximum_geometry_error']
                if key not in expected or key in seen: raise ValueError('incomplete or duplicate Fillet comparison')
                if type(error) not in (int, float) or not math.isfinite(error) or not 0 <= error <= 1e-6: raise ValueError('Fillet geometry gate failed')
                if row['archives'] != 1 or row['changed'] is not rows[key]['history_changed'] or row['success'] is not (result(rows[key]) == 'Success'): raise ValueError('Fillet history or archive gate failed')
                cases[key][mode + '_' + stage + '_maximum_geometry_error'] = error; seen.add(key)
            if seen != expected: raise ValueError('incomplete or duplicate Fillet comparison')
            required = ['cached_candidates_reject_stale_sources_and_invalid_options_without_consuming_redo'] if stage == 'command' else [
                'distance_hover_reuses_the_candidate_and_acceptance_commits_it_once',
                'ready_options_survive_cancellation_and_dynamic_resets_in_the_next_invocation',
                'preview_paints_in_four_views_and_three_display_modes_without_model_edits']
            if any(name not in text for name in required) or ('test result: ok. 2 passed; 0 failed;' if stage == 'command' else 'test result: ok. 4 passed; 0 failed;') not in text: raise ValueError('missing Fillet cache, persistence or viewport coverage')
            if stage == 'ui' and text.count('FILLET_MOUSE_SOURCE_PICKS 2') != 2: raise ValueError('missing viewport mouse routing coverage')
    return cases, diagnostics, captures, counts, paths


def build(proof, revision):
    if re.fullmatch('[0-9a-f]{40}', revision or '') is None: raise ValueError('full implementation revision required')
    git = lambda *args: subprocess.check_output(['git', *args], cwd=ROOT)
    git('cat-file', '-e', revision + '^{commit}'); git('merge-base', '--is-ancestor', BASELINE, revision)
    checks = json.loads((proof / 'checks.json').read_bytes())
    if [(c['name'], c['command']) for c in checks] != list(commands()) or any(c['status'] for c in checks): raise ValueError('incomplete or failed Fillet checks')
    cases, diagnostics, captures, counts, paths = comparisons(proof)
    changed = git('diff', '--name-only', '--diff-filter=ACMR', BASELINE, revision).decode().splitlines()
    if not {'crates/viboceros-command/src/fillet/prepare.rs', 'src/app/curve_fillet.rs', 'src/app/curve_fillet/radius.rs', 'src/app/curve_fillet/input.rs', 'crates/viboceros-smlib/src/native/curve_fillet_contacts.rs', 'crates/viboceros-smlib/tests/fillet_contacts.rs'}.issubset(changed): raise ValueError('missing ordered Fillet implementation')
    return dict(scope='Ordered Fillet picks, dynamic numbers and two-point distances, source identity and attributes, separate current-layer arcs, continuation preferences, cancellation, and shared cached preview/commit geometry.',
                baseline_revision=BASELINE, implementation_revision=revision, full_native_parity=False, **counts,
                retained_failed_captures=2, successful_command_replays_per_build=16, ui_replays_per_build=25,
                archive_round_trips_per_build=41, viewport_combinations_per_build=12, windowless_source_picks_per_build=4, model_epsilon=1e-6,
                maximum_geometry_difference=max(v for c in cases.values() for k, v in c.items() if k.endswith('_maximum_geometry_error')),
                input_evidence='Prescribed source definitions, calibrated public source picks, explicit settings and initial Native getter prompts construct replays. Native output coefficients, domains, PointAt/TangentAt evaluations and pixels are expectations only. The cancellation/default test reuses one registry without reseeding between the captured commands.',
                construction='The shared portable/usd-brep fillet producers generate cached candidates. Native contacts restore the newly constructed arc-length chart after leaving the unit-radius working frame. Retained lines and the joined first-source origin retain their source charts. The same prepared geometry commits atomically; selected originals and temporary prospective pieces are drawn separately during preview.',
                execution='Licensed Rhino captures use private Xvfb displays. Application and viewport tests are windowless.',
                limits=['Four published input-driver diagnostics are excluded: two pilots omitted a radius value; two dynamic cases omitted the final acceptance Enter. A dispatch failure and a later overlapping-pick Selection Menu attempt each published zero observations; their producer closures and diagnostics are retained. The latter family was repeated with separated curved-source picks.',
                        'Geometry qualification covers the retained corpus, not every source or a continuous distance bound. Closed/self-source fillets, arbitrary branch choices, complete invalid-input trajectories, pixels, alternate native display settings and comparative performance remain unqualified. Existing smooth rational-extension and fully consumed added-segment limits remain. Preview scene construction scales with the document size for each changed candidate.'],
                cases=[cases[k] for k in sorted(cases)], diagnostic_cases=diagnostics, captures=captures,
                checks=[dict(name=c['name'], status=c['status']) for c in checks],
                implementation_git_sha256={revision + ':' + p: sha(git('cat-file', 'blob', revision + ':' + p)) for p in changed},
                evidence_sha256={str(p.relative_to(ROOT)): sha(p.read_bytes()) for p in sorted(paths)})


def main():
    p = argparse.ArgumentParser(description=__doc__); p.add_argument('--implementation-revision', required=True); args = p.parse_args()
    (ROOT / 'docs/fillet-getter-validation.json').write_text(json.dumps(build(PROOF, args.implementation_revision), indent=2, allow_nan=False) + '\n')

if __name__ == '__main__': main()
