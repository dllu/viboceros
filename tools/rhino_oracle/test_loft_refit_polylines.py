"""Native corner thresholds, nonlinear line clocks, and point-end references."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_polylines import request as polylines
from .generate_loft_refit_polyline_angles import request as angles
from .generate_loft_refit_polyline_ends import request as ends

HERE = Path(__file__).resolve().parent


class PolylineRefitTests(unittest.TestCase):
    def test_recipes_archives_coverage_and_source_preservation(self):
        for family, recipe, count in [('loft_refit_polylines', polylines, 110),
                ('loft_refit_polyline_angles', angles, 26), ('loft_refit_polyline_ends', ends, 35)]:
            path = HERE / ('fixtures/' + family + '.json')
            query = json.loads(path.read_text())
            data = json.loads((HERE / ('observations/' + family + '.json')).read_text())
            self.assertEqual(query, recipe())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in data['results']])
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), data['provenance']['request_sha256'])
            for name, digest in data['provenance']['sources'].items():
                content = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(content).hexdigest(), digest, name)
            for row in data['results']:
                self.assertEqual(row['value']['before'], row['value']['after'])

    def test_native_corner_threshold_and_mixed_open_profiles(self):
        rows = json.loads((HERE / 'observations/loft_refit_polyline_angles.json').read_text())['results']
        for row in rows:
            surface = row['value']['surfaces'][0]
            self.assertEqual(surface['degree'][1], 3)
            if row['id'].startswith('loft-refit-corner-angle-'):
                angle = float(row['id'][len('loft-refit-corner-angle-'):].split('-')[0])
                self.assertEqual(surface['control_count'][1] == 7, angle > 1.)
            else:
                self.assertIn(surface['control_count'][1], [10, 13])


if __name__ == '__main__':
    unittest.main()
