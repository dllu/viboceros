"""Current sources and measured interactive Sweep1 workflow checks."""
from tools.verify_provenance import verify_measurement
from .qualification_sources import with_shared_sources
import hashlib
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class SweepPickingTests(unittest.TestCase):
    def test_sources_and_verified_workflow_checks(self):
        report = json.loads((ROOT / 'docs/sweep1/picking-validation.json').read_text())
        self.assertTrue(report['interactive_point_getter_implemented'])
        self.assertFalse(report['complete_rhino_sweep_compatibility'])
        for mode in ['portable_ui', 'native_ui']:
            self.assertEqual(report['checks'][mode]['tests'], 10)
        for key in ['portable_images', 'native_images']:
            rows = report[key]
            self.assertEqual(len(rows), 12)
            self.assertEqual(len({(r['family'], r['id']) for r in rows}), 12)
            self.assertTrue(all(0 <= r['complete_image_bound'] <= 1e-7 for r in rows))
        for check in report['checks'].values():
            self.assertEqual(check['exit_code'], 0)
        self.assertGreater(verify_measurement(report, expand_sources=with_shared_sources), 0)
        self.assertIn('src/app/curve_selection.rs', report['sha256'])
        self.assertIn('crates/viboceros-document/src/selection.rs', report['sha256'])


if __name__ == '__main__':
    unittest.main()
