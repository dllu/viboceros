"""Complete admission coverage and source bindings for adaptive Loft commands."""
from tools.verify_provenance import verify_measurement
from .qualification_sources import with_shared_sources
import hashlib
import json
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT / 'tools/rhino_oracle'


def read(path):
    return json.loads(path.read_text())


def surfaces(value):
    if isinstance(value, list):
        return value
    if 'surfaces' in value:
        return value['surfaces']
    return value['outputs'][0]['surfaces']


class AdaptivePreparationValidationTests(unittest.TestCase):
    def setUp(self):
        self.record = read(ROOT / 'docs/adaptive-loft-preparation-validation.json')
        baseline = read(HERE / 'fixtures/loft_refit_preparation_baseline.json')
        self.baseline = {(r['family'], r['id']): r['initial_preparation']
                         for r in baseline['cases']}
        self.families = dict.fromkeys(r['family'] for r in baseline['cases'])
        self.operations = {}
        self.native = {}
        for family in self.families:
            query = read(HERE / ('fixtures/' + family + '.json'))
            native = read(HERE / ('observations/' + family + '.json'))
            self.operations.update(((family, op['id']), op) for op in query['operations'])
            self.native.update(((family, row['id']), row['value']) for row in native['results'])

    def test_every_backend_retains_the_complete_case_and_legacy_sets(self):
        self.assertEqual(len(self.baseline), 1536)
        self.assertEqual(len(self.operations), 1545)
        self.assertEqual(set(self.operations), set(self.native))
        legacy = sum(op['op'] != 'loft' or
                     (self.baseline[key] and bool(surfaces(self.native[key])))
                     for key, op in self.operations.items())
        self.assertEqual(legacy, 873)
        self.assertEqual(set(self.record['configurations']), {'default', 'native_smlib'})
        for label, config in self.record['configurations'].items():
            with self.subTest(backend=label):
                self.assertEqual(set(config['families']), set(self.families))
                self.assertEqual(config['cases'], len(self.operations))
                self.assertEqual(config['legacy'], legacy)
                for field in ['cases', 'strict', 'geometric', 'declined', 'legacy']:
                    self.assertEqual(config[field], sum(row[field] for row in config['families'].values()))
                for family, row in config['families'].items():
                    self.assertEqual(row['cases'], sum(key[0] == family for key in self.operations))
                    self.assertEqual(row['strict'] + row['geometric'] + row['declined'], row['cases'])

    def test_expected_declines_are_explicit_and_do_not_hide_legacy_successes(self):
        saved = read(HERE / 'fixtures/loft_refit_preparation_declines.json')['cases']
        expected = {(r['family'], r['id']) for r in saved}
        recorded = {(r['family'], r['id']): r for r in self.record['expected_declines']}
        self.assertEqual(len(expected), len(saved))
        self.assertEqual(expected, set(recorded))
        self.assertTrue(expected <= set(self.operations))
        for key in expected:
            empty = not surfaces(self.native[key])
            self.assertEqual(recorded[key]['native_returned_empty'], empty)
            if self.baseline[key]:
                self.assertTrue(empty, key)
        self.assertEqual(self.record['native_supported_declines'],
                         sum(not row['native_returned_empty'] for row in recorded.values()))

    def test_declared_sources_bind_the_checked_implementation_and_inputs(self):
        hashes = self.record['sha256']
        self.assertTrue(hashes)
        self.assertGreater(verify_measurement(self.record, expand_sources=with_shared_sources), 0)
        for identity, expected in self.record.get('git_sha256', {}).items():
            data = subprocess.check_output(['git', 'show', identity], cwd=ROOT)
            self.assertEqual(hashlib.sha256(data).hexdigest(), expected, identity)


if __name__ == '__main__':
    unittest.main()
