"""Independent region correspondence, frozen producers, and complete images."""
from tools.verify_provenance import verify_measurement
from .qualification_sources import with_shared_sources
import gzip
import hashlib
import json
from pathlib import Path
import unittest

from .generate_sweep1_section_regions import request
from .generate_sweep1_section_region_holdouts import request as holdouts
from .sweep1_probe import macro, validate

HERE = Path(__file__).resolve().parent


class SweepRegionTests(unittest.TestCase):
    def test_requests_native_outputs_and_producers_are_frozen(self):
        for family, expected in [('sweep1_section_regions', request()),
                                 ('sweep1_section_region_holdouts', holdouts())]:
            with self.subTest(family=family):
                fixture = (HERE / ('fixtures/' + family + '.json')).read_bytes()
                native = json.loads((HERE / ('observations/' + family + '.json')).read_bytes())
                self.assertEqual(json.loads(fixture), expected)
                self.assertEqual(native['engine'], 'rhino')
                self.assertEqual(len(native['results']), 8)
                self.assertEqual([op['id'] for op in expected['operations']],
                                 [row['id'] for row in native['results']])
                self.assertEqual(hashlib.sha256(fixture).hexdigest(), native['provenance']['request_sha256'])
                for digest in native['provenance']['sources'].values():
                    source = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
                    self.assertEqual(hashlib.sha256(source).hexdigest(), digest)

    def test_region_requests_use_the_unsimplified_command_path(self):
        for payload in [request(), holdouts()]:
            for operation in payload['operations']:
                validate(operation)
                self.assertNotIn('refit_tolerance', operation)
                self.assertNotIn('rebuild_point_count', operation)
                text = macro(operation, ['rail', 'first', 'second'])
                self.assertIn('_Simplify=_None', text)
                self.assertNotIn('_Simplify=_Refit', text)
                self.assertNotIn('_RebuildCount=', text)

    def test_historical_region_record_binds_complete_images_and_sources(self):
        root = HERE.parent.parent
        record = json.loads((root / 'docs/sweep1/regions-validation.json').read_bytes())
        self.assertTrue(record['section_region_matching_implemented'])
        self.assertFalse(record['complete_rhino_sweep_compatibility'])
        self.assertEqual(record['comparison_epsilon'], 1e-7)
        for key in ['portable_images', 'native_images']:
            rows = record[key]
            self.assertEqual(len(rows), 16)
            self.assertEqual(len({(r['family'], r['id']) for r in rows}), 16)
            self.assertTrue(all(0 <= r['complete_image_bound'] <= 1e-7 for r in rows))
        for check in record['checks'].values():
            self.assertEqual(check['exit_code'], 0)
        self.assertGreater(verify_measurement(record, expand_sources=with_shared_sources), 0)


if __name__ == '__main__':
    unittest.main()
