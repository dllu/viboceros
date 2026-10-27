"""Keep independent spacing/tolerance holdouts and their native producers fixed."""
import gzip
import hashlib
import json
from pathlib import Path
from unittest import TestCase
from .generate_loft_refit_stage_reserves import request

HERE = Path(__file__).parent


class LoftRefitStageReserveTests(TestCase):
    def test_independent_ratios_tolerances_and_reversal_have_complete_coverage(self):
        fixture = (HERE / 'fixtures/loft_refit_stage_reserves.json').read_bytes()
        query = json.loads(fixture)
        self.assertEqual(query, request())
        self.assertEqual(len(query['operations']), 24)
        self.assertEqual(len({op['id'] for op in query['operations']}), 24)
        self.assertEqual({op['refit_tolerance'] for op in query['operations']}, {2e-7, 4e-7, 8e-7})
        native = json.loads((HERE / 'observations/loft_refit_stage_reserves.json').read_text())
        self.assertEqual([op['id'] for op in query['operations']],
                         [row['id'] for row in native['results']])
        for row in native['results']:
            self.assertEqual(row['value']['before'], row['value']['after'])
            self.assertTrue(row['value']['surfaces'])
        self.assertEqual(hashlib.sha256(fixture).hexdigest(), native['provenance']['request_sha256'])

    def test_native_receipt_preserves_exact_frozen_producers(self):
        native = json.loads((HERE / 'observations/loft_refit_stage_reserves.json').read_text())
        sources = native['provenance']['sources']
        self.assertIn('generate_loft_refit_stage_reserves.py', sources)
        self.assertIn('rhino_worker.py', sources)
        for digest in sources.values():
            archive = HERE / 'observations/source_snapshots' / (digest + '.gz')
            self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), digest)
