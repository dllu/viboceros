"""Native source evidence for certified cubics, independent of Loft Refit."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_cubic_approximation_sources import request

HERE = Path(__file__).resolve().parent


class CubicApproximationTests(unittest.TestCase):
    def test_native_recipes_samples_and_archived_producers(self):
        fixture = HERE / 'fixtures/cubic_approximation_sources.json'
        query = json.loads(fixture.read_text())
        data = json.loads((HERE / 'observations/cubic_approximation_sources.json').read_text())
        self.assertEqual(query, request())
        self.assertEqual(len(query['operations']), 20)
        self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in data['results']])
        self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(), data['provenance']['request_sha256'])
        for name, digest in data['provenance']['sources'].items():
            archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
            self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), digest, name)
        for row in data['results']:
            self.assertEqual(len(row['value']['samples']), 33)
            self.assertEqual(row['value']['samples'][0]['parameter'], row['value']['domain'][0])
            self.assertEqual(row['value']['samples'][-1]['parameter'], row['value']['domain'][1])

    def test_native_hidden_polynomial_has_zero_grid_samples_and_a_real_bump(self):
        rows = json.loads((HERE / 'observations/cubic_approximation_sources.json').read_text())['results']
        value = next(r['value'] for r in rows if r['id'] == 'cubic-source-hidden-bump')
        samples = value['samples']
        self.assertEqual([samples[i]['point'][1] for i in [0, 8, 16, 24, 32]], [0.] * 5)
        self.assertEqual(samples[4]['point'][1], 1.025390625)

    def test_fresh_holdouts_use_non_grid_parameters_and_frozen_producers(self):
        from .generate_cubic_approximation_holdouts import request as holdouts_request
        fixture = HERE / 'fixtures/cubic_approximation_holdouts.json'
        query = json.loads(fixture.read_text())
        data = json.loads((HERE / 'observations/cubic_approximation_holdouts.json').read_text())
        self.assertEqual(query, holdouts_request())
        self.assertEqual(len(query['operations']), 20)
        self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in data['results']])
        self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(), data['provenance']['request_sha256'])
        for name, digest in data['provenance']['sources'].items():
            archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
            self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), digest, name)
        for operation, row in zip(query['operations'], data['results']):
            value = row['value']
            self.assertEqual(len(value['sides']), 11)
            self.assertEqual([s['parameter'] for s in value['sides']], operation['sided_parameters'])
            a, b = value['domain']
            for sample in value['sides']:
                fraction = (sample['parameter'] - a) / (b - a)
                self.assertGreater(min(abs(fraction - i / 32.) for i in range(33)), 1e-6)
                self.assertEqual(sample['left']['point'], sample['right']['point'])


if __name__ == '__main__':
    unittest.main()
