"""Immutable independent input, calibration-only outputs and strict replay reports."""
import base64
from copy import deepcopy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .insert_point_cases import ALL_CASES, request
from .insert_point_mesh_sdk_cases import request as sdk_request
from .report_insert_point_command import (
    CELL_GAPS, FAMILIES, HERE, ROOT, capture, sdk_calibration, summarize,
)
from .structural_sdk import stage_helpers, validate_operations
from .qualification_sources import with_shared_sources


class InsertPointCommandTests(unittest.TestCase):
    def test_shared_source_expansion_binds_extracted_code_in_one_pass(self):
        for root, required in [
            ('src/app.rs', ['src/app/insert_point.rs', 'src/viewport/insert_point.rs']),
            ('src/viewport.rs', ['src/viewport/insert_point.rs']),
            ('crates/viboceros-geometry/src/mesh.rs', [
                'crates/viboceros-geometry/src/mesh/insert/ngons.rs',
                'crates/viboceros-geometry/src/edge_insertion/polygons.rs']),
            ('crates/viboceros-command/src/lib.rs', ['crates/viboceros-command/src/insert_point/arguments.rs']),
        ]:
            paths = with_shared_sources([root])
            self.assertTrue(set(required) <= set(paths))
            self.assertEqual(set(paths), set(with_shared_sources(paths)))

    def test_new_captures_bind_requests_frozen_producers_and_source_archives(self):
        for family in ['insert_point_holdouts_v2', 'insert_point_mesh_sdk', 'insert_point_mesh_sdk_origins']:
            path = HERE/'fixtures'/(family+'.json')
            query = json.loads(path.read_text())
            wanted = request(holdouts=True) if 'holdouts' in family else sdk_request(origins=family.endswith('origins'))
            self.assertEqual(query, wanted)
            validate_operations(query)
            native = capture(family)
            self.assertEqual(native['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            for name, digest in native['provenance']['sources'].items():
                current = (HERE/name).read_bytes()
                frozen = gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(frozen).hexdigest(), digest)
                if hashlib.sha256(current).hexdigest() == digest:
                    self.assertEqual(current, frozen)
            for row in native['results']:
                self.assertNotIn('error', row)
                if family == 'insert_point_holdouts_v2':
                    recipe = deepcopy(row['value']['input'])
                    self.assertEqual(recipe.pop('capture_id'), row['id'])
                    self.assertEqual(recipe, ALL_CASES[row['id']])
                    path = HERE/'fixtures'/family/(row['id']+'.3dm')
                    self.assertEqual(path.read_bytes(), base64.b64decode(row['value']['source_3dm'], validate=True))

    def test_failed_import_is_a_diagnostic_with_no_surviving_response(self):
        failure = json.loads((HERE/'observations/insert_point_holdouts_import_failure.json').read_text())
        self.assertEqual(failure['status'], 'worker_import_failure')
        self.assertEqual(failure['qualified_workflows'], 0)
        self.assertIs(failure['response_retained'], False)
        for field, path in [('request_sha256', 'fixtures/insert_point_holdouts.json'),
                            ('log_sha256', 'observations/insert_point_holdouts_import_failure.log')]:
            self.assertEqual(failure[field], hashlib.sha256((HERE/path).read_bytes()).hexdigest())
        self.assertFalse((HERE/'observations/insert_point_holdouts.json').exists())
        for digest in failure['sources'].values():
            data = gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), digest)

    def test_standalone_new_probes_and_recipe_allowlists(self):
        for query, module in [(request(holdouts=True), 'insert_point_probe'),
                              (sdk_request(), 'insert_point_mesh_sdk_probe'),
                              (sdk_request(origins=True), 'insert_point_mesh_sdk_probe')]:
            for extra in [{'case': 'arbitrary'}, {'id': 'arbitrary'}, {'path': 'arbitrary'}]:
                op = dict(query['operations'][0], **extra)
                with self.assertRaises(ValueError):
                    validate_operations(dict(query, operations=[op]))
            with tempfile.TemporaryDirectory() as directory:
                stage_helpers(query, HERE, Path(directory))
                subprocess.run([sys.executable, '-I', '-c',
                                'import sys;sys.path.insert(0,sys.argv[1]);__import__(sys.argv[2])',
                                directory, module], check=True, capture_output=True)

    def evidence(self):
        report = json.loads((ROOT/'docs/insert-point-command-qualification.json').read_text())
        lines = []
        for stage, key in [('COMMAND', 'cases'), ('GETTER', 'getters'), ('HOLDOUT', 'holdouts')]:
            lines.extend('INSERT_POINT_'+stage+' '+json.dumps(r) for r in report[key])
        return report, lines

    def test_report_reconstructs_all_stages_with_explicit_native_gaps(self):
        report, lines = self.evidence()
        self.assertEqual(summarize('\n'.join(reversed(lines))), report)
        self.assertTrue(report['command_implemented'])
        self.assertEqual(report['command_cases'], 21)
        self.assertEqual(report['getter_cases'], 21)
        self.assertEqual(report['additional_geometry_cases'], 12)
        self.assertEqual(report['primary_cell_gaps'], sorted(CELL_GAPS))
        self.assertEqual(len(report['holdout_region_gaps']), 2)
        self.assertFalse(report['full_native_parity'])
        self.assertFalse(report['performance_qualified'])
        self.assertEqual(sum(FAMILIES.values()), 89)

    def test_calibration_does_not_substitute_for_command_input_evidence(self):
        rows = sdk_calibration()
        self.assertEqual(len(rows), 16)
        self.assertEqual(sum(r['api_matches']['polyline_triangles_quads_0.0'] for r in rows), 7)
        self.assertEqual(sum(r['api_matches']['closed_polyline_quads_0.0'] for r in rows), 1)
        self.assertEqual(sum(bool(r['matching_origins']) for r in rows), 7)
        report, _ = self.evidence()
        self.assertEqual(report['sdk_calibration'], rows)
        self.assertEqual(report['calibration_recipes'], 32)

    def test_missing_duplicate_nonfinite_and_relabelled_replays_fail(self):
        report, lines = self.evidence()
        for text in ['\n'.join(lines[1:]), '\n'.join(lines+lines[:1])]:
            with self.assertRaises(ValueError):
                summarize(text)
        row = report['cases'][0]
        for field, value in [('history', False), ('preview', False), ('identity', True+0),
                             ('changed', not row['changed'])]:
            bad = deepcopy(row)
            bad[field] = value
            with self.assertRaises(ValueError):
                summarize('\n'.join(['INSERT_POINT_COMMAND '+json.dumps(bad)]+lines[1:]))
        for field, value in [('maximum_error', float('nan')), ('maximum_error', float('inf')),
                             ('maximum_error', 1e-3), ('vertices', True), ('faces', -1),
                             ('channels', False), ('logical_regions', False),
                             ('stored_pieces_correspond', True), ('stored_facets_correspond', True),
                             ('native_cells', []), ('actual_cells', [])]:
            bad = deepcopy(row)
            bad['comparison'][field] = value
            with self.assertRaises(ValueError):
                summarize('\n'.join(['INSERT_POINT_COMMAND '+json.dumps(bad)]+lines[1:]))
        bad = dict(row, id='unrelated')
        with self.assertRaises(ValueError):
            summarize('\n'.join(['INSERT_POINT_COMMAND '+json.dumps(bad)]+lines[1:]))
        gap = next(r for r in report['holdouts'] if not r['comparison']['qualified'])
        for flag in ['qualified', 'logical_regions']:
            forged = deepcopy(gap)
            forged['comparison'][flag] = True
            text = '\n'.join(s for s in lines if not s.startswith('INSERT_POINT_HOLDOUT '+json.dumps(gap)))
            with self.assertRaises(ValueError):
                summarize(text+'\nINSERT_POINT_HOLDOUT '+json.dumps(forged))


if __name__ == '__main__':
    unittest.main()
