"""Reconstruct native ArcBlend handle and cursor-metric qualification."""
import argparse
import gzip
import hashlib
import json
import math
import re
import subprocess
from .arc_blend_handle_evidence import collect
from .check_arc_blend_handles import ROOT, PROOF, commands
from .report_arc_blend import records

BASELINE = '8ff4c34d9ad7d1700ee41544e27d861820243f93'

def build(proof, revision):
    if re.fullmatch('[0-9a-f]{40}', revision or '') is None:
        raise ValueError('full implementation revision required')
    git = lambda *args: subprocess.check_output(['git', *args], cwd=ROOT)
    git('cat-file', '-e', revision + '^{commit}')
    git('merge-base', '--is-ancestor', BASELINE, revision)
    checks = json.loads((proof / 'checks.json').read_bytes())
    if [(c['name'], c['command']) for c in checks] != list(commands()) or any(c['status'] for c in checks):
        raise ValueError('incomplete or failed checks')
    matched, diagnostics, captures, physical, queries, evidence = collect(proof)
    cases = {key: dict(id=key) for key in matched}
    stations = {}
    for mode in ['portable', 'native']:
        for stage in ['command', 'ui']:
            log = 'handles-' + stage + '-' + mode
            for row in records(proof, log, 'ARC_BLEND_HANDLE_' + stage.upper(), matched, 'maximum_coefficient_and_sample_error', archives=True):
                cases[row['id']][mode + '_' + stage + '_error'] = row['maximum_coefficient_and_sample_error']
            text = gzip.decompress((proof / (log + '.log.gz')).read_bytes()).decode()
            rows = [json.loads(line) for line in re.findall(r'^ARC_BLEND_HANDLE_VIEW_LINE (.+)$', text, re.M)]
            if len(rows) != 4 or {r['marker'] for r in rows} != set(queries):
                raise ValueError('incomplete viewing-line queries')
            for row in rows:
                if type(row['parameter']) not in [int, float] or not math.isfinite(row['parameter']):
                    raise ValueError('invalid view-line parameter')
                difference = row['screen_metric_station_difference']
                if queries[row['marker']]:
                    if type(difference) not in [int, float] or not math.isfinite(difference) or difference < 0:
                        raise ValueError('invalid public metric comparison')
                    old = stations.setdefault(row['marker'], difference)
                    if abs(difference - old) > 1e-10:
                        raise ValueError('metric comparison changed across replays')
                elif difference is not None:
                    raise ValueError('unexpected public projection matrix')
        text = gzip.decompress((proof / ('handles-command-' + mode + '.log.gz')).read_bytes()).decode()
        rows = [json.loads(line) for line in re.findall(r'^ARC_BLEND_HANDLE_DIAGNOSTIC (.+)$', text, re.M)]
        if len(rows) != len(diagnostics) or {r['id']: r['reason'] for r in rows} != diagnostics:
            raise ValueError('incomplete or changed diagnostic replays')
    if stations['@catenary-click:arc_chord_spatial_perspective_mouse-3'] < 1e-4:
        raise ValueError('Perspective metric distinction disappeared')
    paths = git('diff', '--name-only', '--diff-filter=ACMR', BASELINE, revision).decode().splitlines()
    if not {'crates/viboceros-command/src/arc_blend/handles.rs', 'src/app/arc_blend/adjustment.rs', 'src/viewport/arc_blend.rs', 'src/app/tests/arc_blend_handles.rs'}.issubset(paths):
        raise ValueError('implementation lacks native handle workflow')
    sha = lambda b: hashlib.sha256(b).hexdigest()
    recipes = {json.dumps(v['prescribed_case'], sort_keys=True) for v in matched.values()}
    return dict(
        scope='Source endpoint sliding and chord-based positive control-polygon ratios, modal typed/mouse adjustments, held drags, cached previews, Escape rollback and final Trim/Join commits.',
        baseline_revision=BASELINE, implementation_revision=revision,
        model_epsilon=1e-6, full_native_parity=False,
        native_observations=44, matched_native_observations_per_build=26,
        distinct_matched_input_recipes=len(recipes), retained_diagnostics=18,
        native_physical_picks_in_matched_workflows=physical,
        archive_round_trips_per_build=52, view_line_queries_per_stage_per_build=4,
        independent_public_projection_comparisons_per_stage_per_build=2,
        public_projection_station_differences=stations,
        maximum_coefficient_and_local_sample_difference=max(c[k] for c in cases.values() for k in c if k.endswith('_error')),
        cases=[cases[k] for k in sorted(cases)],
        diagnostics=[dict(id=key, reason=diagnostics[key]) for key in sorted(diagnostics)],
        captures=captures, checks=[dict(name=c['name'], status=c['status']) for c in checks],
        construction='Original source curves determine endpoint positions and tangents. Free ratio picks use the public viewing ray and existing Move/Copy line constraint; typed points use 3D chord projection. Native output geometry is expected data only.',
        execution='All native captures used private Xvfb sessions. Egui clicks, held drags, cancellation and stale-reference checks are windowless.',
        limits=[
            'RadiusDifference and zero-ratio/end-snap automatic solving remain unsupported. New failed-target probes show nontrivial provisional geometry changes despite native warnings; they remain explicit diagnostics.',
            'Four retained captures have an incomplete driver, three reject input outside the intended adjustment stage, two never select the intended handle and nine request automatic solving. None counts as a passing geometry comparison.',
            'Native endpoint adjustments cover line sources and clamping. Arbitrary nonlinear source contacts and edge adjustments have no native command qualification.',
            'The controlled scene uses unique object names for identity-based comparison. Rhino replaces runtime serial numbers when trimming; object enumeration order is not compared. Native source ID retention is observed separately through guide flags.',
            'Native command comparisons cover NURBS coefficients, knots, domains, attributes, selection and 33 local curve evaluations, plus Undo/Redo and 3DM. Independent native command PointAt sampling, continuous distance and comparative speed remain unqualified.',
            'General native handle hit-box admission, exact tie/depth ranking, SubCrv and remembered defaults remain unqualified or unsupported.'
        ],
        implementation_git_sha256={revision + ':' + p: sha(git('cat-file', 'blob', revision + ':' + p)) for p in paths},
        evidence_sha256={str(p.relative_to(ROOT)): sha(p.read_bytes()) for p in sorted(evidence)},
    )

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--implementation-revision', required=True)
    args = parser.parse_args()
    (ROOT / 'docs/arc-blend-handles-validation.json').write_text(json.dumps(build(PROOF, args.implementation_revision), indent=2, allow_nan=False) + '\n')

if __name__ == '__main__':
    main()
