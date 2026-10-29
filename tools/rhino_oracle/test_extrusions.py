"""Fixed native extrusion sources and fail-closed interchange qualification."""
from tools.verify_provenance import reconstruct_measurement, verify_measurement
import base64
import copy
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from . import tosubd_cases as cases
from .report_extrusions import ROOT, HERE, FAMILIES, observations, summarize
from .structural_sdk import validate_operations


class ExtrusionTests(unittest.TestCase):
    def test_fixed_native_recipes_files_and_archived_producers(self):
        for family, count in FAMILIES:
            query = json.loads((HERE / 'fixtures' / (family + '.json')).read_text())
            self.assertEqual(query, cases.request(**{('pipes' if family == 'tosubd_pipes' else 'extrusions'): True}))
            validate_operations(query)
            native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
            self.assertEqual(len(native['results']), count)
            self.assertEqual(native['provenance']['request_sha256'], hashlib.sha256((HERE / 'fixtures' / (family + '.json')).read_bytes()).hexdigest())
            for row in native['results']:
                self.assertEqual(row['value']['input'], cases.all_cases()[row['id']])
                for source in row['value']['source_files']:
                    self.assertEqual(source['source'], 0)
                    data = (HERE / 'fixtures/extrusions' / (row['id'] + '.3dm')).read_bytes()
                    self.assertEqual(base64.b64decode(source['base64'], validate=True), data)
            for name, sha in native['provenance']['sources'].items():
                data = (HERE / name).read_bytes()
                if hashlib.sha256(data).hexdigest() != sha:
                    data = gzip.decompress((HERE / 'observations/source_snapshots' / (sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(), sha, name)
        self.assertEqual(len(observations()[0]), 49)

    def lines(self):
        report = json.loads((ROOT / 'docs/extrusion-interop-qualification.json').read_text())
        lines = ['EXTRUSION_NATIVE_FILE ' + json.dumps(r) for r in report['comparisons']]
        lines += ['EXTRUSION_NATIVE_REJECTED ' + json.dumps(r) for r in report['rejected']]
        return report, lines

    def test_complete_report_is_historical_and_order_independent(self):
        report, lines = self.lines()
        self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(lines)), report)
        self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(reversed(lines))), report)
        self.assertEqual(report['qualified_file_sources'], 11)
        for key in ['tosubd_command_qualified', 'tosubd_getter_qualified', 'full_native_parity', 'performance_qualified']:
            self.assertIs(report[key], False)
        self.assertGreater(verify_measurement(report), 0)

    def test_missing_duplicate_or_invalid_file_comparisons_cannot_qualify(self):
        report, lines = self.lines()
        for data in [lines[:-1], lines + lines[:1]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(data))
        row = report['comparisons'][0]
        original = 'EXTRUSION_NATIVE_FILE ' + json.dumps(row)
        for key, bad in [('identity_retained', False), ('native_type_retained', False),
                         ('rational_control_nets_retained', False), ('maximum_rational_weight_error', 1.1e-12),
                         ('maximum_rational_control_error', 1.1e-6),
                         ('native_surface_stations_and_orientation', False), ('faces', True),
                         ('maximum_surface_sample_error', float('nan')),
                         ('maximum_surface_sample_error', 1.1e-6), ('solid', 1)]:
            changed = dict(row, **{key: bad})
            data = [s if s != original else 'EXTRUSION_NATIVE_FILE ' + json.dumps(changed) for s in lines]
            with self.assertRaises(ValueError, msg=key):
                summarize('\n'.join(data))


if __name__ == '__main__':
    unittest.main()
