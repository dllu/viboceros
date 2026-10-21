"""Fixed SubD evidence, archived capture producers and complete qualification."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from . import face_create_probe as probe
from .report_subd import GROUPS, ROOT, summarize
from .structural_sdk import validate_operations

HERE = Path(__file__).resolve().parent


class SubDTests(unittest.TestCase):
    def test_captured_requests_and_producer_provenance(self):
        for name, factory in zip(GROUPS, [probe.subd_request, probe.subd_surface_request]):
            path = HERE / 'fixtures' / (name + '.json')
            request = json.loads(path.read_text())
            self.assertEqual(request, factory())
            validate_operations(request)
            response = json.loads((HERE / 'observations' / (name + '.json')).read_text())
            self.assertEqual(response['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            self.assertEqual([r['id'] for r in response['results']], [o['id'] for o in request['operations']])
            for source, sha in response['provenance']['sources'].items():
                data = (HERE / source).read_bytes()
                if hashlib.sha256(data).hexdigest() != sha:
                    data = gzip.decompress((HERE / 'observations/source_snapshots' / (sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(), sha)
            for row in response['results']:
                value = row['value']
                self.assertEqual(value['input'], probe.SCENES[row['id']])
                self.assertTrue(value['success'])
                self.assertEqual(value['before'], [])
                self.assertEqual(value['undo'], [])
                self.assertEqual(value['after_script'], value['redo'])
                self.assertTrue(value['after_script'][0]['subd']['valid'])
                self.assertNotIn('Unknown command:', value['history'])

    def lines(self):
        report = json.loads((ROOT / 'docs/subd-qualification.json').read_text())
        lines = ['SUBD_' + stage.upper() + ' ' + json.dumps(row)
                 for stage, rows in report['stages'].items() for row in rows]
        return report, lines

    def test_current_source_maps_and_complete_qualification(self):
        report, lines = self.lines()
        self.assertEqual(summarize('\n'.join(lines)), report)
        self.assertEqual(report['model_epsilon'], 1e-6)
        self.assertEqual(report['recipes'], 10)
        self.assertEqual(report['surface_mesh_cases'], 10)
        self.assertFalse(report['full_native_parity'])
        self.assertFalse(report['performance_qualified'])
        for path, sha in report['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), sha, path)

    def test_missing_duplicate_failed_and_invalid_rows_cannot_qualify(self):
        report, lines = self.lines()
        for invalid in [lines[:-1], lines + lines[-1:], lines[::-1]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(invalid))
        for key, bad in [('maximum_sampled_surface_error', float('nan')),
                         ('maximum_control_or_limit_point_error', True),
                         ('maximum_sampled_surface_error', 1.1e-6),
                         ('vertices', 0), ('qualified', False),
                         ('attributes_and_selection', False), ('surface_sampled', True)]:
            row = copy.deepcopy(report['stages']['command'][0])
            row['check'][key] = bad
            invalid = lines[:]
            invalid[0] = 'SUBD_COMMAND ' + json.dumps(row)
            with self.assertRaises(ValueError):
                summarize('\n'.join(invalid))


if __name__ == '__main__':
    unittest.main()
