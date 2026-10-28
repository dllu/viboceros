"""Immutable controlled profiles separate fitting policy from input shape."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest

from .generate_sweep1_refit_common_handles import request

HERE = Path(__file__).resolve().parent


class CommonHandleCaptureTests(unittest.TestCase):
    def test_native_requests_and_producers_are_frozen(self):
        fixture = (HERE / 'fixtures/sweep1_refit_common_handles.json').read_bytes()
        native = json.loads((HERE / 'observations/sweep1_refit_common_handles.json').read_bytes())
        self.assertEqual(json.loads(fixture), request())
        self.assertEqual(native['engine'], 'rhino')
        self.assertEqual(len(native['results']), 16)
        self.assertEqual([op['id'] for op in request()['operations']],
                         [row['id'] for row in native['results']])
        self.assertEqual(hashlib.sha256(fixture).hexdigest(), native['provenance']['request_sha256'])
        for digest in native['provenance']['sources'].values():
            source = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(source).hexdigest(), digest)

    def test_identical_profile_pairs_use_a_different_native_fit(self):
        observations = json.loads((HERE / 'observations/sweep1_refit_common_handles.json').read_bytes())
        rows = {row['id']: row['value'][0]['definition'] for row in observations['results']}
        single = rows['common-0.99-1-0.01']
        pair = rows['common-0.99-2-0.01']
        self.assertEqual(single['control_count'][1], 4)
        self.assertEqual(pair['control_count'][1], 4)
        # The same authored profile lies at the rail start in both recipes.
        a = single['control_points'][single['control_count'][0]]['point']
        b = pair['control_points'][pair['control_count'][0]]['point']
        self.assertGreater(abs(a[0] - b[0]), 1e-4)


if __name__ == '__main__':
    unittest.main()
