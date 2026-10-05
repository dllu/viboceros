"""Retained native evidence for continuously certified cutting p-curves."""
import hashlib
import json
import math
from pathlib import Path
from unittest import TestCase

ROOT = Path(__file__).resolve().parents[2]


class CertifiedSurfaceSplitTests(TestCase):
    def test_private_capture_provenance_and_sources_are_retained(self):
        provenance = json.loads((ROOT / 'docs/certified-surface-splits-provenance.json').read_text())
        self.assertTrue(provenance['private_xvfb'])
        self.assertFalse(provenance['full_native_parity'])
        self.assertEqual(provenance['settings_scheme'], 'VibocerosOracleCertifiedSplit_20261005')
        self.assertEqual(provenance['engine_version'], '8.32.26160.13001')
        self.assertEqual((provenance['native_command_recipes'], provenance['result_faces']), (2, 4))
        for path, digest in provenance['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest, path)

    def test_complete_outputs_keep_topology_selection_and_trim_geometry(self):
        fixture = json.loads((ROOT / 'tools/rhino_oracle/fixtures/surface_split_nonaffine_trimmed.json').read_text())
        capture = json.loads((ROOT / 'tools/rhino_oracle/observations/certified_surface_split.json').read_text())
        self.assertEqual([r['id'] for r in capture['results']], [op['id'] for op in fixture['operations']])
        for op, row in zip(fixture['operations'], capture['results']):
            value = row['value']
            self.assertTrue(value['command_succeeded'])
            self.assertEqual(value['cutters_selected'], [False])
            self.assertEqual(len(value['objects']), 2)
            for obj in value['objects']:
                for field in ('attributes_match_source', 'in_source_group', 'selected'):
                    self.assertTrue(obj[field], row['id'])
                self.assertFalse(obj['original_id'])
                self.assertEqual(obj['object_kind'], 'brep')
                self.assertEqual(obj['surface']['control_points'], op['surface']['control_points'])
                self.assertEqual(obj['topology']['edge_count'], 4)
                self.assertEqual(obj['topology']['vertex_count'], 4)
                self.assertFalse(obj['topology']['is_solid'])
                self.assertEqual(len(obj['trim_curves']), 1)
                for key in ('uv_points', 'surface_points'):
                    points = obj['trim_curves'][0][key]
                    self.assertEqual(len(points), 65)
                    self.assertTrue(all(math.isfinite(x) for p in points for x in p))
