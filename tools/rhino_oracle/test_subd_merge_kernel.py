"""Independent cleanup/priority holdouts and complete SubD kernel qualification."""
import base64
import gzip
import hashlib
import json
import unittest
from pathlib import Path
from .generate_subd_merge_faces_holdouts import request as holdouts
from .generate_subd_merge_faces_cleanup import request as cleanup
from .generate_subd_merge_faces_valence import request as valence
from .qualification_sources import SUBD_MERGE_MODULES, with_shared_sources
from .structural_sdk import validate_operations
from .test_subd_merge_faces import subd

HERE = Path(__file__).resolve().parent
FAMILIES = [('subd_merge_faces_holdouts', holdouts, 32),
            ('subd_merge_faces_cleanup', cleanup, 14), ('subd_merge_faces_valence', valence, 7)]


def results(family):
    return json.loads((HERE / 'observations' / (family + '.json')).read_text())


class SubDMergeKernelTests(unittest.TestCase):
    def test_holdout_recipes_provenance_archives_and_native_history(self):
        changed, unchanged = 0, 0
        for family, make, count in FAMILIES:
            query = make(); validate_operations(query)
            fixture = HERE / 'fixtures' / (family + '.json')
            self.assertEqual(json.loads(fixture.read_text()), query)
            result = results(family)
            self.assertEqual(len(result['results']), count)
            self.assertEqual([r['id'] for r in result['results']], [op['id'] for op in query['operations']])
            self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(), result['provenance']['request_sha256'])
            for source, digest in result['provenance']['sources'].items():
                data = (HERE / source).read_bytes()
                if hashlib.sha256(data).hexdigest() != digest:
                    data = gzip.decompress((HERE / 'observations/source_snapshots' / (digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(), digest, source)
            for row in result['results']:
                value = row['value']; before = subd(value['before']); after = subd(value['after'])
                self.assertEqual(value['command_result'], 'Success', row['id'])
                for phase in ['before', 'after', 'undone', 'redone']:
                    self.assertTrue(subd(value[phase])['subd']['valid'], (row['id'], phase))
                self.assertEqual(subd(value['undone'])['subd'], before['subd'], row['id'])
                self.assertEqual(subd(value['redone'])['subd'], after['subd'], row['id'])
                for key in ['source', 'name', 'layer', 'groups', 'color', 'color_source', 'grips_on']:
                    self.assertEqual(before[key], after[key], (row['id'], key))
                for key in ['source_3dm', 'after_3dm']:
                    self.assertTrue(base64.b64decode(value[key], validate=True).startswith(b'3D Geometry File Format'))
                if before['subd'] == after['subd']:
                    unchanged += 1; self.assertEqual(len(value['undone']), 1)
                else:
                    changed += 1; self.assertEqual(len(value['undone']), 2)
        self.assertEqual((changed, unchanged), (51, 2))

    def test_corner_cleanup_and_overlapping_request_priority(self):
        data = {r['id']: r['value'] for r in results('subd_merge_faces_holdouts')['results']}
        after = lambda name: subd(data[name]['after'])['subd']
        for name in ['faces-overlap-edge', 'edge-overlap-faces', 'faces-overlap-vertex', 'vertex-overlap-faces']:
            self.assertEqual(after(name)['vertex_ids'], [1, 4, 5, 6, 7, 8], name)
        for name in ['face-between-edges', 'face-overlap-vertex']:
            self.assertEqual(subd(data[name]['before'])['subd'], after(name), name)
        self.assertEqual(after('tri-faces-corner-first')['vertex_tags'][0], 3)
        self.assertEqual(after('strip-faces-corner-all')['vertex_tags'], [3, 3, 3, 3])
        extra = {r['id']: r['value'] for r in results('subd_merge_faces_cleanup')['results']}
        self.assertEqual(subd(extra['ignored-edge-with-vertex']['after'])['subd']['vertex_ids'], [1, 2, 3, 4, 7, 8])
        self.assertEqual(subd(extra['ignored-edge-with-vertex']['after'])['selected_edges'], [2])
        self.assertEqual(subd(extra['edge-opposite-vertex']['after'])['subd']['vertex_ids'], [1, 4, 5, 6, 7])

    def test_valence_four_boundary_junctions_and_interior_cleanup(self):
        data = {r['id']: r['value'] for r in results('subd_merge_faces_valence')['results']}
        for name in ['collinear', 'convex', 'concave']:
            after = subd(data['boundary-valence-four-' + name]['after'])['subd']
            self.assertIn(1, after['vertex_ids'])
            self.assertEqual(len(after['faces'][0]), 3)
        for name in ['smooth', 'dart', 'crease', 'side-crease']:
            after = subd(data['grid-L-' + name]['after'])['subd']
            self.assertNotIn(5, after['vertex_ids'])
            self.assertEqual(len(after['faces']), 2)

    def test_kernel_qualification_from_every_implementation_entry(self):
        for source in SUBD_MERGE_MODULES:
            self.assertTrue(set(SUBD_MERGE_MODULES) <= set(with_shared_sources([source])), source)

    def test_validation_receipt_binds_current_code_and_all_native_families(self):
        root = HERE.parent.parent
        receipt = json.loads((root / 'docs/geometry/subd-face-merge-validation.json').read_text())
        self.assertTrue(receipt['command_registered'])
        self.assertEqual(receipt['native_cases'], 94)
        for name, digest in receipt['sources'].items():
            self.assertEqual(hashlib.sha256((root / name).read_bytes()).hexdigest(), digest, name)
        self.assertTrue(set(SUBD_MERGE_MODULES) <= set(receipt['sources']))
        self.assertEqual(set(receipt['families']), {'subd_merge_faces', *[family for family, _, _ in FAMILIES]})
        for family, hashes in receipt['families'].items():
            for folder, digest in hashes.items():
                self.assertEqual(hashlib.sha256((HERE / folder / (family + '.json')).read_bytes()).hexdigest(), digest)
