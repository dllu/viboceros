"""Fresh native sources, local image construction, and retained contract gaps."""
import hashlib
import json
import math
from pathlib import Path
from unittest import TestCase

from .surface_curve_image_probe import request

ROOT = Path(__file__).resolve().parents[2]


def read(path):
    return json.loads((ROOT / path).read_text())


class SurfacePushupCertifiedTests(TestCase):
    def test_fixture_retains_every_original_native_surface_and_uv_definition(self):
        capture = read('tools/rhino_oracle/observations/surface_pushup_certified.json')
        fixture = read('tools/rhino_oracle/fixtures/surface_pushup_certified.json')
        self.assertEqual([r['id'] for r in capture['results']],
                         [op['id'] for op in request()['operations']])
        self.assertEqual(len(fixture['operations']), 13)
        for row, op in zip(capture['results'], fixture['operations']):
            self.assertEqual(op['id'], row['id'])
            self.assertEqual(op['op'], 'surface_pushup_certified')
            self.assertEqual(op['limit'], 1e-6)
            source = row['value']
            self.assertTrue(source['sources_unchanged'])
            self.assertEqual(op['parameter_curve'], source['parameter_curve'])
            surface = source['surface']
            self.assertEqual([op['surface']['degree_u'], op['surface']['degree_v']], surface['degree'])
            self.assertEqual([op['surface']['control_point_count_u'], op['surface']['control_point_count_v']],
                             surface['control_count'])
            for field in ('control_points', 'knots_u', 'knots_v', 'domain_u', 'domain_v'):
                self.assertEqual(op['surface'][field], surface[field])

    def test_local_images_certify_all_sources_and_keep_native_parameterization_gaps(self):
        local = read('docs/certified-surface-pushup-local.json')
        fixture = read('tools/rhino_oracle/fixtures/surface_pushup_certified.json')
        self.assertEqual([r['id'] for r in local['results']], [op['id'] for op in fixture['operations']])
        for row, op in zip(local['results'], fixture['operations']):
            value = row['value']
            self.assertTrue(value['certified'])
            self.assertEqual(value['correspondence'], 'normalized_curve_domains')
            self.assertTrue(math.isfinite(value['bound']) and 0 <= value['bound'] <= op['limit'])
            self.assertEqual(value['spatial_curve']['domain'], op['parameter_curve']['domain'])
        native = read('tools/rhino_oracle/observations/surface_pushup_certified.json')
        pushups = [r['value'] for r in native['results'] if r['value']['pushup']]
        self.assertEqual(len(pushups), 5)
        self.assertTrue(all(v['maximum_locus_error'] <= 1e-6 for v in pushups))
        seam = next(r['value'] for r in native['results'] if r['id'].endswith('sphere_seam'))
        self.assertGreater(seam['maximum_sample_error'], .006)
        self.assertEqual(len(seam['samples']), 129)

    def test_capture_provenance_retains_private_display_and_exact_source_hashes(self):
        provenance = read('docs/certified-surface-pushup-provenance.json')
        self.assertTrue(provenance['private_xvfb'])
        self.assertFalse(provenance['full_native_parity'])
        self.assertEqual(provenance['settings_scheme'], 'VibocerosOraclePushup20261006')
        self.assertEqual((provenance['sdk_recipes'], provenance['native_pushup_recipes'],
                          provenance['certified_local_images']), (13, 5, 13))
        for path, digest in provenance['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest, path)
