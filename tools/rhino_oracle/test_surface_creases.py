"""Frozen native crease probes and direct derivative witnesses of missed bends."""
import gzip
import hashlib
import json
import math
from pathlib import Path
import unittest
from .generate_surface_creases import request, holdout_request, witness_request

HERE = Path(__file__).resolve().parent
FAMILIES = [('surface_creases_smoke', lambda: request(True), 4),
            ('surface_creases', request, 96), ('surface_creases_holdouts', holdout_request, 12),
            ('surface_creases_witnesses', witness_request, 5)]


class SurfaceCreaseTests(unittest.TestCase):
    def test_qualification_source_bindings(self):
        root = HERE.parents[1]
        record = json.loads((root / 'docs/surface-creases-validation.json').read_text())
        self.assertEqual(record['native_records'], 117)
        self.assertEqual(record['raw_topology_matches'], 98)
        self.assertEqual(record['additional_exact_partitions'], 19)
        self.assertFalse(record['full_rhino_parity'])
        self.assertFalse(record['continuous_angle_certificate'])
        for name, digest in record['sha256'].items():
            self.assertEqual(hashlib.sha256((root / name).read_bytes()).hexdigest(), digest, name)

    def test_recipes_sources_coverage_validity_and_source_purity(self):
        count = 0
        for family, recipe, expected in FAMILIES:
            fixture = HERE / ('fixtures/' + family + '.json')
            query = json.loads(fixture.read_text())
            native = json.loads((HERE / ('observations/' + family + '.json')).read_text())
            self.assertEqual(query, recipe())
            self.assertEqual(len(query['operations']), expected)
            self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in native['results']])
            self.assertEqual(native['provenance']['request_sha256'], hashlib.sha256(fixture.read_bytes()).hexdigest())
            for name, digest in native['provenance']['sources'].items():
                archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
                self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), digest, name)
            for row in native['results']:
                c = row['value']['crease']
                self.assertTrue(c['succeeded'] and c['valid'] and c['source_unchanged'])
                self.assertEqual(c['initial_faces'], 1)
                self.assertTrue(all(len(s['samples']) == 289 for s in c['surfaces']))
                self.assertEqual(len(c['surfaces']), len(c['face_reversed']))
                count += 1
        self.assertEqual(count, 117)

    def test_native_splitter_exceeds_the_basic_query_and_retains_a_below_limit_control(self):
        rows = json.loads((HERE / 'observations/surface_creases_smoke.json').read_text())['results']
        for row, faces, cuts in zip(rows, [2, 1, 2, 2], [0, 0, 1, 0]):
            c = row['value']['crease']
            self.assertEqual(len(c['surfaces']), faces)
            self.assertEqual(sum(map(len, c['raw_breaks'])), cuts)

    def test_rhinos_own_one_sided_derivatives_witness_its_unsplit_narrow_bends(self):
        query = json.loads((HERE / 'fixtures/surface_creases_witnesses.json').read_text())
        native = json.loads((HERE / 'observations/surface_creases_witnesses.json').read_text())
        exceeded = below = 0
        for op, row in zip(query['operations'], native['results']):
            a, b = [s['du'] for s in row['value']['samples']]
            cosine = sum(x * y for x, y in zip(a, b)) / math.hypot(*a) / math.hypot(*b)
            measured = math.acos(max(-1., min(1., cosine)))
            self.assertEqual(len(row['value']['crease']['surfaces']), 1)
            self.assertEqual(row['value']['crease']['raw_breaks'], [[], []])
            if measured > op['inspect_crease']:
                self.assertGreater(measured - op['inspect_crease'], 1e-5)
                exceeded += 1
            else:
                below += 1
        self.assertEqual((exceeded, below), (4, 1))


if __name__ == '__main__':
    unittest.main()
