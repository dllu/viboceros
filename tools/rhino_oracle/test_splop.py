"""Splop captures retain owned command, input, target and Undo/Redo evidence."""
import copy
import gzip
import hashlib
import json
import tempfile
from pathlib import Path
from unittest import TestCase
from .generate_splop import request
from .structural_sdk import stage_helpers, validate_operations
HERE=Path(__file__).resolve().parent


class SplopTests(TestCase):
    def test_corpus_requests_and_transitive_sources_are_retained(self):
        q=json.loads((HERE/'fixtures/splop.json').read_text())
        r=json.loads((HERE/'observations/splop.json').read_text())
        self.assertEqual(q,request())
        self.assertEqual(len(q['operations']),134)
        self.assertEqual(len(r['results']),134)
        digest=lambda b:hashlib.sha256(b).hexdigest()
        self.assertEqual(digest((HERE/'fixtures/splop.json').read_bytes()),r['provenance']['request_sha256'])
        for name,expected in r['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if digest(data)!=expected:
                data=gzip.decompress((HERE/'observations/source_snapshots'/(expected+'.gz')).read_bytes())
            self.assertEqual(digest(data),expected,name)
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(q,HERE,Path(folder))
            self.assertTrue((Path(folder)/'splop_probe.py').is_file())
            for p in Path(folder).iterdir():self.assertEqual(p.read_bytes(),(HERE/p.name).read_bytes())

    def test_completed_and_cancelled_copy_batches_have_real_history_and_unchanged_guides(self):
        q=request()
        r=json.loads((HERE/'observations/splop.json').read_text())
        for op,row in zip(q['operations'],r['results']):
            with self.subTest(id=op['id']):
                self.assertEqual(op['id'],row['id'])
                self.assertNotIn('error',row)
                value=row['value']
                self.assertEqual(value['success'],op.get('finish')!='Cancel')
                self.assertTrue(value['guide_unchanged'])
                self.assertEqual(value['recording'],dict(enabled=True,active=False))
                self.assertIn('Undoing Splop',value['undo_history'])
                self.assertIn('Redoing Splop',value['undo_history'])
                self.assertNotIn('Nothing to undo',value['undo_history'])
                self.assertEqual(len(value['undo']['objects']),len(value['before']['objects']))
                self.assertEqual(len(value['redo']['objects']),len(value['after_script']['objects']))
                self.assertTrue(value['after']['guide_selected'])
                self.assertFalse(value['after_script']['guide_selected'])
                self.assertFalse(value['redo']['guide_selected'])
                expected_copies=1+len(op.get('targets',[])) if op['copy'] else 0
                self.assertEqual(len(value['after_script']['objects']),len(value['before']['objects'])*(1+expected_copies))

    def test_non_idle_dispatch_diagnostic_is_kept_separate_from_validated_corpus(self):
        diagnostic=json.loads(gzip.decompress((HERE.parent.parent/'docs/splop-dispatch-undo-diagnostic.json.gz').read_bytes()))
        self.assertEqual(len(diagnostic['results']),134)
        self.assertTrue(all('Nothing to undo' in row['value']['undo_history'] for row in diagnostic['results']))
        for name,expected in diagnostic['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=expected:
                data=gzip.decompress((HERE/'observations/source_snapshots'/(expected+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),expected)

    def test_invalid_spheres_flags_shapes_and_batches_are_rejected_without_launch(self):
        base=request()['operations'][0]
        for patch in [dict(radius=0.),dict(radius=float('nan')),dict(angle=float('inf')),dict(shape='Other'),dict(surface='Other'),dict(copy=1),dict(rigid='Yes'),dict(reference_point=1),dict(locked_radius=-1.),dict(targets=[[2.,0.]]),dict(extra=True)]:
            with self.subTest(patch=patch),self.assertRaises(ValueError):
                validate_operations(dict(operations=[dict(copy.deepcopy(base),**patch)]))
