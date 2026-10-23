"""Immutable insertion producers and narrowly scoped SubD kernel evidence."""
import base64
from copy import deepcopy
import gzip
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from .insert_point_cases import ALL_CASES, request
from .report_insert_point import FAMILIES, HERE, ROOT, observations, summarize
from .structural_sdk import stage_helpers, validate_operations


class InsertPointTests(unittest.TestCase):
    def test_producer_snapshots_requests_and_exact_source_files(self):
        flags = {'insert_point_discovery': {}, 'insert_point_live': {'live': True},
                 'insert_point_full': {'full': True}, 'insert_point_regions': {'regions': True}}
        for family, count in FAMILIES.items():
            path = HERE/'fixtures'/(family+'.json')
            query = json.loads(path.read_text())
            self.assertEqual(query, request(**flags[family]))
            validate_operations(query)
            capture = json.loads((HERE/'observations'/(family+'.json')).read_text())
            self.assertEqual(len(capture['results']), count)
            self.assertEqual(capture['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            for name, digest in capture['provenance']['sources'].items():
                data = (HERE/name).read_bytes()
                if hashlib.sha256(data).hexdigest() != digest:
                    data = gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(), digest, name)
            for row in capture['results']:
                value = row.get('value')
                if value is None:
                    self.assertTrue(row.get('error'))
                    continue
                recipe = deepcopy(value['input'])
                if 'capture_id' in recipe:
                    self.assertEqual(recipe.pop('capture_id'), row['id'])
                self.assertEqual(recipe, ALL_CASES[row['id']])
                source = HERE/'fixtures'/family/(row['id']+'.3dm')
                self.assertEqual(source.read_bytes(), base64.b64decode(value['source_3dm'], validate=True))

    def test_fixed_allowlist_and_standalone_helpers(self):
        query = request(full=True)
        for change in [dict(query['operations'][0], case='arbitrary'), dict(query['operations'][0], path='arbitrary'), dict(query['operations'][0], id='arbitrary')]:
            with self.assertRaises(ValueError):
                validate_operations(dict(query, operations=[change]))
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(query, HERE, Path(directory))
            subprocess.run([sys.executable, '-I', '-c', 'import sys;sys.path.insert(0,sys.argv[1]);import insert_point_probe', directory], check=True, capture_output=True)

    def evidence(self):
        report = json.loads((ROOT/'docs/insert-point-kernel-qualification.json').read_text())
        return report, ['INSERT_POINT_KERNEL '+json.dumps(r) for r in report['cases']]

    def test_report_reconstructs_without_command_or_mesh_parity_claims(self):
        report, lines = self.evidence()
        self.assertEqual(summarize('\n'.join(reversed(lines))), report)
        self.assertEqual(report['subd_kernel_cases'], 10)
        self.assertEqual(report['captured_workflows'], 45)
        self.assertFalse(report['command_implemented'])
        self.assertFalse(report['full_native_parity'])
        self.assertFalse(report['performance_qualified'])
        expected, excluded, _ = observations()
        self.assertEqual(len(expected), 10)
        self.assertEqual(len(excluded), 35)
        self.assertIn('Incomplete', excluded['insert_point_regions_side_2_75'])

    def test_missing_duplicate_excessive_and_forged_evidence_fail(self):
        report, lines = self.evidence()
        for text in ['\n'.join(lines[:-1]), '\n'.join(lines+lines[-1:])]:
            with self.assertRaises(ValueError):
                summarize(text)
        for key, bad in [('qualified', False), ('vertices', True), ('surface_sampled', False), ('maximum_sampled_surface_error', float('nan')), ('maximum_control_or_limit_point_error', float('inf')), ('maximum_sampled_surface_error', 1e-3)]:
            row = deepcopy(report['cases'][0])
            row['comparison'][key] = bad
            with self.assertRaises(ValueError):
                summarize('\n'.join(['INSERT_POINT_KERNEL '+json.dumps(row)]+lines[1:]))
        row = dict(report['cases'][0], id='unrelated')
        with self.assertRaises(ValueError):
            summarize('\n'.join(['INSERT_POINT_KERNEL '+json.dumps(row)]+lines[1:]))


if __name__ == '__main__':
    unittest.main()
