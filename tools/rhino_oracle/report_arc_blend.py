"""Reconstruct the bounded first ArcBlend implementation qualification."""
import argparse
import gzip
import hashlib
import json
import math
import re
import subprocess
from .arc_blend_evidence import collect
from .check_arc_blend import ROOT, PROOF, commands

BASELINE = '79e3c842e1f930651a5e1e899dd3d935d0abcfe3'

def records(proof, log, marker, names, field=None, archives=False):
    text = gzip.decompress((proof / (log + '.log.gz')).read_bytes()).decode()
    rows = [json.loads(line) for line in re.findall('^' + marker + r' (.+)$', text, re.M)]
    if len(rows) != len(names) or {r['id'] for r in rows} != set(names):
        raise ValueError('incomplete or duplicate ' + marker + ' replay')
    for row in rows:
        if field:
            value = row[field]
            if type(value) not in [int, float] or not math.isfinite(value) or not 0 <= value <= 1e-6:
                raise ValueError('geometry gate failed')
        if archives and row['archives'] != 1:
            raise ValueError('archive gate failed')
    return rows

def build(proof, revision):
    if re.fullmatch('[0-9a-f]{40}', revision or '') is None:
        raise ValueError('full implementation revision required')
    git = lambda *args: subprocess.check_output(['git', *args], cwd=ROOT)
    git('cat-file', '-e', revision + '^{commit}')
    git('merge-base', '--is-ancestor', BASELINE, revision)
    checks = json.loads((proof / 'checks.json').read_bytes())
    if [(c['name'], c['command']) for c in checks] != list(commands()) or any(c['status'] for c in checks):
        raise ValueError('incomplete or failed checks')
    matched, rejected, diagnostics, workflows, captures, evidence = collect(proof)
    cases = {name: dict(id=name) for name in matched | workflows}
    log = 'arc-geometry'
    for row in records(proof, log, 'ARC_BLEND_SDK', matched, 'maximum_point_and_tangent_error'):
        cases[row['id']]['shared_sdk_error'] = row['maximum_point_and_tangent_error']
    records(proof, log, 'ARC_BLEND_REJECTION', rejected)
    sdk_diagnostics = {k for k in diagnostics if k.startswith('spatial_')}
    rows = records(proof, log, 'ARC_BLEND_DIAGNOSTIC', sdk_diagnostics)
    for row in rows:
        value = row['native_join_tangent_difference']
        if type(value) not in [int, float] or not math.isfinite(value) or abs(value - diagnostics[row['id']]['native_join_tangent_difference']) > 1e-12:
            raise ValueError('SDK diagnostic replay changed')
    records(proof, log, 'ARC_BLEND_COMMAND_GEOMETRY', {'arc_default', 'arc_s_shape', 'arc_spatial', 'arc_alternate'}, 'maximum_coefficient_error')
    for mode in ['portable', 'native']:
        for stage in ['command', 'ui']:
            for row in records(proof, 'arc-' + stage + '-' + mode, 'ARC_BLEND_' + stage.upper(), workflows, 'maximum_coefficient_and_sample_error', archives=True):
                cases[row['id']][mode + '_' + stage + '_error'] = row['maximum_coefficient_and_sample_error']
    paths = git('diff', '--name-only', '--diff-filter=ACMR', BASELINE, revision).decode().splitlines()
    if not {'crates/viboceros-geometry/src/arc_blend.rs', 'crates/viboceros-command/src/arc_blend.rs', 'src/app/arc_blend.rs', 'src/app/tests/arc_blend.rs'}.issubset(paths):
        raise ValueError('implementation lacks ArcBlend geometry command or interaction')
    sha = lambda b: hashlib.sha256(b).hexdigest()
    return dict(
        scope='Positive-ratio tangent biarcs, alternate signed solution, ordered open curve/edge references, cached preview, explicit contact parameters, Trim and Join for distinct ordinary curves.',
        baseline_revision=BASELINE, implementation_revision=revision,
        model_epsilon=1e-6, full_native_parity=False,
        native_observations=53, matching_sdk_constructions=20,
        matching_sdk_rejections=17, matching_native_workflows_per_build=6,
        native_physical_picks_in_matched_workflows=12, archive_round_trips_per_build=12,
        independent_native_point_and_tangent_samples=660,
        maximum_sdk_point_tangent_and_coefficient_difference=max(c[k] for c in cases.values() for k in c if k.endswith('_sdk_error')),
        maximum_command_coefficient_and_local_sample_difference=max(c[k] for c in cases.values() for k in c if k.endswith('_command_error') or k.endswith('_ui_error')),
        cases=[cases[k] for k in sorted(cases)],
        rejected_sdk_inputs=sorted(rejected), diagnostics=[dict(id=k, **diagnostics[k]) for k in sorted(diagnostics)],
        captures=captures, checks=[dict(name=c['name'], status=c['status']) for c in checks],
        construction='Independent normalized tangent-length equation; exact circular arc and OpenNURBS rational conversion. Native result coordinates and coefficients are expected values only.',
        execution='All native captures used private Xvfb sessions; egui interaction tests are windowless.',
        limits=[
            'RadiusDifference, zero-ratio automatic solving, draggable adjustment handles, SubCrv and remembered Trim/Join defaults remain unimplemented. Ratio and Parameter1/2 are explicit Viboceros scripting options.',
            'Native command captures qualify endpoint contacts on lines. Interior trimming, arbitrary nonlinear source picks, surface-edge references, curved self-blends and contact parameter options have no native command qualification.',
            'The geometry constructor normalizes direction vectors. Three retained native SDK observations with non-unit directions have non-tangent joins and are diagnostics, not passing comparisons.',
            'Seven command discoveries contain incomplete input, unknown options or unattained radius targets. They are retained and classified, not counted as matched workflows.',
            'Zero-length, straight and degenerate two-arc constructions fail. Joined self-blends and surface-edge trimming are rejected explicitly.',
            'Thirty-three independent Native PointAt and TangentAt samples qualify each of 20 SDK constructions. Command comparisons use NURBS coefficients and 33 local evaluations. Continuous distance, extreme scaling and comparative performance remain unqualified.'
        ],
        implementation_git_sha256={revision + ':' + p: sha(git('cat-file', 'blob', revision + ':' + p)) for p in paths},
        evidence_sha256={str(p.relative_to(ROOT)): sha(p.read_bytes()) for p in sorted(evidence)},
    )

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--implementation-revision', required=True)
    args = parser.parse_args()
    (ROOT / 'docs/arc-blend-validation.json').write_text(json.dumps(build(PROOF, args.implementation_revision), indent=2, allow_nan=False) + '\n')

if __name__ == '__main__':
    main()
