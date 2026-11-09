"""Reproduce refinement limits from owned Native captures and rejected trials."""
import json

from .surface_average_adaptive_evidence import ROOT, PROOF, BASELINE, collect, prototypes, sha
from .report_surface_average_refinement import build as earlier


def build(proof=PROOF):
    rows, commands, paths = collect(proof)
    models, model_paths = prototypes(proof)
    paths.update(model_paths)
    previous = ROOT / 'docs/surface-average-refinement-investigation.json'
    prior = json.loads(previous.read_bytes())
    if prior != earlier():
        raise ValueError('prior refinement investigation changed')
    paths.add(previous)
    signs = []
    for shift in [-.5, -.2, 0., .2, .5, 1.]:
        keys = ['average_refine_signs_' + mode + '_' + str(shift) for mode in ['tangency', 'curvature']]
        counts = [[rows[key][side]['surfaces'][0]['control_count'][1] for side in ['output', 'target']] for key in keys]
        signs.append(dict(source_translation_z=shift, tangency_counts_v=counts[0], curvature_counts_v=counts[1]))
    position = {key: dict(degrees=[value[side]['surfaces'][0]['degree'] for side in ['output', 'target']],
                          counts=[value[side]['surfaces'][0]['control_count'] for side in ['output', 'target']])
                for key, value in rows.items() if key.startswith('average_position_refine_')}
    return dict(
        scope='Adaptive averaged refinement investigation; all refinement remains unsupported.',
        baseline_revision=BASELINE, new_headless_launches=6, new_native_sdk_cases=len(rows),
        new_native_command_cases=len(commands), cumulative_native_primary_observations=prior['native_primary_observations'] + len(rows) + len(commands),
        frozen_prototypes=models, averaged_refinement_qualified=False,
        continuous_continuity_qualified=False, performance_qualified=False,
        curvature_translation_sweep=signs, position_outputs=position,
        fitting_stage_outputs={key: dict(degrees=[value[side]['surfaces'][0]['degree'] for side in ['output', 'target']],
                                         counts=[value[side]['surfaces'][0]['control_count'] for side in ['output', 'target']])
                               for key, value in rows.items() if key.startswith('average_fit_stage_')},
        one_sided_diagnostics={key: dict(degree=value['output']['surfaces'][0]['degree'],
                                         count=value['output']['surfaces'][0]['control_count'], qualified=False)
                               for key, value in rows.items() if key.startswith('average_fit_one_sided_')},
        public_prefit_queries=sum(len(value.get('prefit_audit', [])) for value in rows.values()),
        null_public_prefit_results=sum(q['output'] is None for value in rows.values() for q in value.get('prefit_audit', [])),
        distance_tolerance_readbacks={key: dict(requested=value['prescribed_case']['distance_tolerance'],
                                               native=value['configured_settings']['distance'])
                                     for key, value in rows.items() if key.startswith('average_position_refine_weights_')},
        native_commands={key: dict(accepted=value['success'], qualified=False,
                                  undo_geometry_restored=True, redo_restored=True,
                                  physical_clicks=len(value['input_trace'][0]['clicks'])) for key, value in commands.items()},
        findings=[
            'A curvature sign policy improves the coupled pilot model from 25/30 to 27/30. Three polynomial cases still fail; the model remains rejected.',
            'Source translations toggle Native curvature refinement while all six tangency pairs retain four transverse controls. This supports a sign-related stopping hypothesis, not a general proven rule.',
            'The position prototype matches 30/37 cases. Closest-point matching, several natural-edge charts, reversed traversal and unequal transverse bases expose seven failures.',
            'Native position refinement raises the degree-2 source transverse basis to degree 3, producing 10 and 12 controls. Some chart cases need 43 to 51 transverse controls.',
            'Four physical command acceptances refine both supports to seven transverse controls. Cancel preserves the inputs, and a subsequent command recalls the refinement options.',
            'A requested SDK distance tolerance of 1e-8 reads back as 1.490116119385e-8; the requested and configured settings are retained separately.',
            'A loose refinement tolerance still raises the degree-2 transverse basis to degree 3. Pre-elevated inputs give identical degree/count inventories across the recorded tolerance sweep.',
            'Public MatchToCurve keeps degree 2 on the original unequal-basis source and selects different knot counts. It does not reproduce the missing averaged refinement stage.',
            'One-sided MatchSrf on the elevated source returns 11 controls where averaged matching returns 10 and 12. These 16 diagnostic cases have no qualified Rust replay.',
        ],
        limits=[
            'No new averaged refinement capability is enabled. The existing 1e-9 geometry gate is unchanged.',
            'Frozen Rust replays use the portable backend only. No Native backend replay, continuous-continuity or performance qualification is claimed.',
            'Grid errors for rejected position cases are diagnostics; a small sampled error does not replace the complete support, boundary, knot and archive comparison.',
            'The first position trial stopped at its first failure. Its log is retained separately and is not counted as a full-scope replay.',
            'The earlier catalog retains the original failed Native launch and four invalid composed calls.',
        ],
        evidence_sha256={str(path.relative_to(ROOT)): sha(path.read_bytes()) for path in sorted(paths)},
    )


if __name__ == '__main__':
    value = build()
    (ROOT / 'docs/surface-average-adaptive-investigation.json').write_text(json.dumps(value, indent=2, allow_nan=False) + '\n')
    print('Recorded adaptive averaging investigation; production remains unqualified')
