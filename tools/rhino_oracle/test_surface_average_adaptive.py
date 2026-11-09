"""Keep the broad scope, failed models and physical command evidence intact."""
import copy
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path

from .surface_average_adaptive_evidence import ROOT, PROOF, collect, prototypes, command_complete, sha
from .surface_average_refinement_evidence import complete
from .report_surface_average_adaptive import build


class AdaptiveAverageTests(unittest.TestCase):
    def test_catalog_reproduces_without_enabling_refinement(self):
        value = build()
        self.assertEqual(value, json.loads((ROOT / 'docs/surface-average-adaptive-investigation.json').read_bytes()))
        self.assertEqual((value['new_native_sdk_cases'], value['new_native_command_cases'],
                          value['cumulative_native_primary_observations']), (92, 5, 199))
        self.assertFalse(value['averaged_refinement_qualified'])
        self.assertEqual({k: v['matched_cases'] for k, v in value['frozen_prototypes'].items()}, {'sign': 27, 'position': 30})
        self.assertEqual(len(value['frozen_prototypes']['position']['rejected_cases']), 7)
        for prototype in value['frozen_prototypes'].values():
            self.assertFalse(prototype['qualified'])
        for path, digest in value['evidence_sha256'].items():
            self.assertEqual(sha((ROOT / path).read_bytes()), digest, path)

    def test_missing_duplicate_and_desktop_captures_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            proof = Path(directory) / 'proof'
            shutil.copytree(PROOF, proof)
            path = proof / 'captures.json'
            original = json.loads(path.read_bytes())
            variants = [original[:-1], original + original[-1:]]
            for key, bad in [('command', ['desktop-launcher']), ('exit_status', False),
                             ('observation_sha256', '0' * 64), ('kind', 'command')]:
                changed = copy.deepcopy(original)
                changed[-1][key] = bad
                variants.append(changed)
            for changed in variants:
                path.write_text(json.dumps(changed))
                with self.assertRaises(ValueError):
                    collect(proof)

    def test_native_readback_clamp_does_not_rewrite_requested_settings(self):
        rows, _, _ = collect()
        for flags in ['10', '01']:
            value = rows['average_position_refine_weights_' + flags + '_1e-08']
            self.assertEqual(value['prescribed_case']['distance_tolerance'], 1e-8)
            self.assertEqual(value['configured_settings']['distance'], 1.490116119385e-8)
            complete(value)
            changed = copy.deepcopy(value)
            changed['configured_settings']['distance'] = 1e-8
            with self.assertRaises(ValueError):
                complete(changed)

    def test_degree_changes_and_large_native_knot_counts_remain_visible(self):
        value = build()['position_outputs']
        self.assertEqual(value['average_position_refine_transverse_basis'],
                         dict(degrees=[[3, 3], [3, 3]], counts=[[4, 10], [4, 12]]))
        self.assertEqual(value['average_position_refine_source_west']['counts'], [[4, 43], [4, 51]])
        self.assertEqual(value['average_position_refine_closest']['counts'], [[4, 28], [4, 42]])

    def test_prefit_and_one_sided_diagnostics_stay_distinct(self):
        value = build()
        self.assertEqual(value['public_prefit_queries'], 96)
        self.assertGreater(value['null_public_prefit_results'], 0)
        self.assertEqual(len(value['one_sided_diagnostics']), 16)
        self.assertTrue(all(not row['qualified'] for row in value['one_sided_diagnostics'].values()))
        stages = value['fitting_stage_outputs']
        for distance in [10., .01, .001, .0001, 1e-6, 1e-8]:
            suffix = '_' + str(distance)
            self.assertEqual(stages['average_fit_stage_transverse_basis' + suffix],
                             stages['average_fit_stage_elevated_basis' + suffix])
        self.assertEqual(stages['average_fit_stage_transverse_basis_10.0'],
                         dict(degrees=[[3, 3], [3, 3]], counts=[[4, 4], [4, 4]]))

    def test_command_cancellation_click_acknowledgements_and_history_are_checked(self):
        _, rows, _ = collect()
        for key, field in [('match_average_position_refine_cancel', 'success'),
                           ('match_average_position_refine', 'click'),
                           ('match_average_position_refine_recall', 'undo'),
                           ('match_average_position_refine_recall', 'option')]:
            value = copy.deepcopy(rows[key])
            if field == 'success':
                value['success'] = True
            elif field == 'click':
                value['input_trace'][0]['clicks'][0]['click_acknowledged'] = False
            elif field == 'undo':
                value['undo'][1]['attributes']['name'] = 'changed'
            else:
                value['input_trace'][0]['prompts'][-1] = value['input_trace'][0]['prompts'][-1].replace('RefineMatch=Yes', 'RefineMatch=No')
            with self.assertRaises(ValueError):
                command_complete(value, value['prescribed_case'])

    def test_rejected_models_cannot_be_promoted_or_lose_sources_and_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            proof = Path(directory) / 'proof'
            shutil.copytree(PROOF, proof)
            for label in ['sign', 'position']:
                path = proof / 'prototypes' / (label + '.json')
                original = path.read_bytes()
                record = json.loads(original)
                for key, bad in [('qualified', True), ('exit_status', 0), ('sources', {}), ('inputs', {}),
                                 ('baseline_revision', '0' * 40)]:
                    changed = copy.deepcopy(record)
                    changed[key] = bad
                    path.write_text(json.dumps(changed))
                    with self.assertRaises(ValueError):
                        prototypes(proof)
                path.write_bytes(original)

    def test_failed_rows_and_error_gate_cannot_be_removed_from_position_trial(self):
        with tempfile.TemporaryDirectory() as directory:
            proof = Path(directory) / 'proof'
            shutil.copytree(PROOF, proof)
            path = proof / 'prototypes/position.log.gz'
            lines = gzip.decompress(path.read_bytes()).decode().splitlines()
            index = next(i for i, line in enumerate(lines) if line.startswith('POSITION_REFINEMENT '))
            changed = lines[:]
            row = json.loads(changed[index].split(' ', 1)[1])
            row['maximum_geometry_error'] = 1e-3
            changed[index] = 'POSITION_REFINEMENT ' + json.dumps(row)
            variants = [changed, lines[:index] + lines[index + 1:], lines + [lines[index]],
                        [line for line in lines if 'Pending position cases: ' not in line]]
            for variant in variants:
                path.write_bytes(gzip.compress('\n'.join(variant).encode(), mtime=0))
                with self.assertRaises(ValueError):
                    prototypes(proof)


if __name__ == '__main__':
    unittest.main()
