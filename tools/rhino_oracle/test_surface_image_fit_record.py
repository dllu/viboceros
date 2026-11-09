"""Keep source bounds, native image gates and input inventories auditable."""
import copy
import json
import unittest
from . import report_surface_image_fit as record


class SurfaceImageFitRecordTests(unittest.TestCase):
    def test_frozen_summary_reproduces(self):
        stored = json.loads((record.ROOT / 'docs/surface-image-fit-validation.json').read_bytes())
        self.assertEqual(record.report(), stored)

    def test_both_complete_native_certificates_keep_the_same_strict_gate(self):
        self.assertEqual(record.geometric_bound(None, 0.), 0.)
        self.assertEqual(record.geometric_bound(1e-10, None), 1e-10)
        for a, b in [(None, 1e-8), (1e-8, 0.), (-1., None), (None, float('nan'))]:
            with self.assertRaises(AssertionError):record.geometric_bound(a, b)

    def test_modified_image_row_bounds_and_ids_are_rejected(self):
        captures = record.adaptive.data()
        log = record.read(record.PROOF / 'portable.log.gz').decode()
        rows = record.adaptive.rows(log, 'SURFACE_IMAGE_FIT')
        def replace(changed):
            lines = iter(changed)
            return '\n'.join('SURFACE_IMAGE_FIT '+json.dumps(next(lines)) if line.startswith('SURFACE_IMAGE_FIT ') else line
                             for line in log.splitlines())
        changed = copy.deepcopy(rows)
        changed[0]['id'] = changed[1]['id']
        with self.assertRaises(AssertionError):record.summarize(captures, replace(changed))
        changed = copy.deepcopy(rows)
        next(r for r in changed if r['source_bound'] is not None)['source_bound'] = 2.
        with self.assertRaises(AssertionError):record.summarize(captures, replace(changed))


if __name__ == '__main__':unittest.main()
