"""Automatic Sweep1 evidence distinguishes full bounds, samples and open gaps."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_sweep1_automatic_curved import request as curved
from .generate_sweep1_automatic_stations import request as stations
from .generate_sweep1_automatic_direction import request as direction
from .qualification_sources import with_shared_sources

ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT / 'tools/rhino_oracle'


class AutomaticSweepTests(unittest.TestCase):
    def test_native_requests_and_archived_producers_are_immutable(self):
        for family, recipe, count in [('sweep1_automatic_curved', curved, 8),
                                       ('sweep1_automatic_stations', stations, 6),
                                       ('sweep1_automatic_direction', direction, 7)]:
            with self.subTest(family=family):
                encoded = (HERE / ('fixtures/' + family + '.json')).read_bytes()
                query = json.loads(encoded)
                native = json.loads((HERE / ('observations/' + family + '.json')).read_text())
                self.assertEqual(query, recipe())
                self.assertEqual(len(query['operations']), count)
                self.assertEqual(native['engine'], 'rhino')
                self.assertEqual(native['engine_version'], '8.32.26160.13001')
                self.assertEqual(hashlib.sha256(encoded).hexdigest(), native['provenance']['request_sha256'])
                self.assertEqual([op['id'] for op in query['operations']], [row['id'] for row in native['results']])
                for name, digest in native['provenance']['sources'].items():
                    data = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
                    self.assertEqual(hashlib.sha256(data).hexdigest(), digest, name)
                for row in native['results']:
                    self.assertEqual(len(row['value']), 1)
                    self.assertEqual(len(row['value'][0]['samples']), 81)
                    d = row['value'][0]['definition']
                    self.assertEqual(len(d['control_points']), d['control_count'][0] * d['control_count'][1])

    def test_native_direction_is_not_a_half_parameter_or_endpoint_distance_guess(self):
        rows = json.loads((HERE / 'observations/sweep1_automatic_direction.json').read_text())['results']
        ends = {}
        for row in rows:
            d = row['value'][0]['definition']
            ends[row['id']] = d['control_points'][d['control_count'][0] - 1]['point']
        for name, expected in [('line-height-4.0', [0., 0., 0.]),
                               ('spatial-direction-0.51', [5., 2., 3.]),
                               ('spatial-direction-0.55', [0., 0., 0.])]:
            for a, b in zip(ends[name], expected):
                self.assertAlmostEqual(a, b, delta=1e-12)

    def test_current_record_binds_sources_and_keeps_measurement_scopes_distinct(self):
        record = json.loads((ROOT / 'docs/sweep1-automatic-validation.json').read_text())
        self.assertEqual(set(record['sha256']), set(with_shared_sources(list(record['sha256']))))
        for name, digest in record['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / name).read_bytes()).hexdigest(), digest, name)
        self.assertEqual(record['straight_surfaces']['native_command_cases'], 7)
        self.assertEqual(record['straight_surfaces']['constructions_per_backend'], 21)
        self.assertEqual(record['curved_samples']['native_command_cases'], 8)
        self.assertEqual(record['curved_samples']['known_frame_gaps'], 1)
        self.assertEqual(record['extent_checks']['cases'], 12)
        self.assertFalse(record['complete_rhino_sweep_compatibility'])


if __name__ == '__main__':
    unittest.main()
