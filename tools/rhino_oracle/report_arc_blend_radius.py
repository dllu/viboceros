"""Reconstruct the bounded parallel-tangent RadiusDifference qualification."""
import argparse
import gzip
import hashlib
import json
import re
import subprocess
from .arc_blend_radius_evidence import collect
from .check_arc_blend_radius import ROOT, PROOF, commands
from .report_arc_blend import records

BASELINE = '23d4c57b32e577c8965f8f3960a0ec9a94e8af89'

def build(proof, revision):
    if re.fullmatch('[0-9a-f]{40}', revision or '') is None:
        raise ValueError('full implementation revision required')
    git = lambda *args: subprocess.check_output(['git', *args], cwd=ROOT)
    git('cat-file', '-e', revision + '^{commit}')
    git('merge-base', '--is-ancestor', BASELINE, revision)
    checks = json.loads((proof / 'checks.json').read_bytes())
    if [(c['name'], c['command']) for c in checks] != list(commands()) or any(c['status'] for c in checks):
        raise ValueError('incomplete or failed checks')
    matched, diagnostics, captures, physical, evidence = collect(proof)
    cases = {key: dict(id=key) for key in matched}
    for mode in ['portable', 'native']:
        for stage in ['command', 'ui']:
            log = 'radius-' + stage + '-' + mode
            for row in records(proof, log, 'ARC_BLEND_RADIUS_' + stage.upper(), matched, 'maximum_coefficient_and_sample_error', archives=True):
                cases[row['id']][mode + '_' + stage + '_error'] = row['maximum_coefficient_and_sample_error']
            for row in records(proof, log, 'ARC_BLEND_RADIUS_' + stage.upper(), matched, 'maximum_native_arc_sample_error'):
                cases[row['id']][mode + '_' + stage + '_native_sample_error'] = row['maximum_native_arc_sample_error']
                if stage == 'command' and row['warning'] != ('Unable to achieve requested radius difference' in matched[row['id']]['history']):
                    raise ValueError('radius warning changed')
        rows = records(proof, 'radius-command-' + mode, 'ARC_BLEND_RADIUS_DIAGNOSTIC', diagnostics)
        if {r['id']: r['reason'] for r in rows} != diagnostics:
            raise ValueError('diagnostic classification changed')
    paths = git('diff', '--name-only', '--diff-filter=ACMR', BASELINE, revision).decode().splitlines()
    if not {'crates/viboceros-geometry/src/arc_blend/radius.rs', 'src/app/arc_blend/radius.rs', 'crates/viboceros-command/tests/arc_blend_radius.rs', 'src/app/tests/arc_blend_radius.rs'}.issubset(paths):
        raise ValueError('implementation lacks radius workflow')
    sha = lambda b: hashlib.sha256(b).hexdigest()
    return dict(
        scope='Signed radius differences for parallel forward tangents, bounded unattainable targets, contact locking, numeric option prompt and target recall through zero ratios and end snaps within one command.',
        baseline_revision=BASELINE, implementation_revision=revision, model_epsilon=1e-6, full_native_parity=False,
        native_observations=41, matched_native_workflows_per_build=18, retained_diagnostics=23,
        matched_native_unattainable_target_warnings_per_build=2,
        native_physical_picks_in_matched_workflows=physical, archive_round_trips_per_build=36,
        independent_native_point_and_tangent_samples=1782,
        maximum_coefficient_and_local_sample_difference=max(c[k] for c in cases.values() for k in c if k.endswith('_error') and not k.endswith('_native_sample_error')),
        maximum_independent_native_arc_point_tangent_and_radius_difference=max(c[k] for c in cases.values() for k in c if k.endswith('_native_sample_error')),
        cases=[cases[k] for k in sorted(cases)], diagnostics=[dict(id=k, reason=diagnostics[k]) for k in sorted(diagnostics)],
        captures=captures, checks=[dict(name=c['name'], status=c['status']) for c in checks],
        construction='Closed-form parallel-tangent radius equation from prescribed source endpoints and directions; native geometry is expected data only. Public ArcCurve and PolyCurve PointAt/TangentAt measurements qualify normalized local stations.',
        execution='Private Xvfb captures and windowless egui tests; no desktop GUI window.',
        limits=[
            'General nonparallel tangent solving remains unsupported. Twenty-one diagnostic captures include both attained targets and failed targets with changed native previews; none counts as a matching workflow.',
            'Two captures inherit a target from an earlier native command. Viboceros recalls targets within the active command; cross-command settings remain unsupported.',
            'The analytical solver admits forward, parallel unit directions within a fixed 1e-12 angular guard, nonstraight chords and the ordinary branch. Explicit positive ratios still use the general biarc constructor.',
            'Native captures cover line source contacts, one translated spatial chord, scaling, signed targets, contact movement, ratio overrides and bounded failed targets. General nonlinear sources, continuous distance and speed comparisons remain unqualified.',
            'Historical creation and handle validation records retain their original scope and diagnostics. SubCrv, general hit-box/depth ranking and remembered Trim/Join defaults remain unsupported or unqualified.'
        ],
        implementation_git_sha256={revision + ':' + p: sha(git('cat-file', 'blob', revision + ':' + p)) for p in paths},
        evidence_sha256={str(p.relative_to(ROOT)): sha(p.read_bytes()) for p in sorted(evidence)},
    )

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--implementation-revision', required=True)
    args = parser.parse_args()
    (ROOT / 'docs/arc-blend-radius-validation.json').write_text(json.dumps(build(PROOF, args.implementation_revision), indent=2, allow_nan=False) + '\n')

if __name__ == '__main__':
    main()
