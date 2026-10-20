"""Retained Splop grip and source-sphere command evidence and staging closure."""
import copy
import gzip
import hashlib
import json
import tempfile
from pathlib import Path
from unittest import TestCase
from .generate_splop_grips import request as grips
from .generate_splop_reference import request as reference
from .structural_sdk import stage_helpers,validate_operations
HERE=Path(__file__).resolve().parent
class SplopExtensionsTests(TestCase):
    def test_inputs_producers_and_transitive_staging_are_retained(self):
        for name,make,count in [('splop_grips',grips,148),('splop_reference',reference,21)]:
            q=json.loads((HERE/'fixtures'/f'{name}.json').read_text())
            r=json.loads((HERE/'observations'/f'{name}.json').read_text())
            self.assertEqual(q,make());self.assertEqual(len(r['results']),count)
            digest=lambda data:hashlib.sha256(data).hexdigest()
            self.assertEqual(digest((HERE/'fixtures'/f'{name}.json').read_bytes()),r['provenance']['request_sha256'])
            for source,expected in r['provenance']['sources'].items():
                data=(HERE/source).read_bytes()
                if digest(data)!=expected:data=gzip.decompress((HERE/'observations/source_snapshots'/f'{expected}.gz').read_bytes())
                self.assertEqual(digest(data),expected,source)
            with tempfile.TemporaryDirectory() as folder:
                stage_helpers(q,HERE,Path(folder))
                self.assertTrue((Path(folder)/f'{name}_probe.py').is_file())
                for p in Path(folder).iterdir():self.assertEqual(p.read_bytes(),(HERE/p.name).read_bytes())
    def test_every_owned_command_has_real_undo_redo_and_matching_object_lifecycle(self):
        for name,make in [('splop_grips',grips),('splop_reference',reference)]:
            r=json.loads((HERE/'observations'/f'{name}.json').read_text())
            for op,row in zip(make()['operations'],r['results']):
                with self.subTest(id=op['id']):
                    self.assertEqual(row['id'],op['id']);self.assertNotIn('error',row)
                    v=row['value'];self.assertEqual(v['success'],op.get('finish')!='Cancel')
                    self.assertIn('Undoing Splop',v['undo_history']);self.assertIn('Redoing Splop',v['undo_history'])
                    self.assertNotIn('Nothing to undo',v['undo_history'])
                    self.assertEqual(len(v['before']['objects']),len(v['undo']['objects']))
                    self.assertEqual(len(v['after_script']['objects']),len(v['redo']['objects']))
                    self.assertFalse(v['after_script']['guide_selected'])
                    if name=='splop_reference':self.assertTrue(v['guide_unchanged']);self.assertIn(" '_-SelID",v['executed_macro'])
                    if name=='splop_grips' and op['copy']:
                        for phase in ['after','after_script','undo','redo']:
                            source=next(o for o in v[phase]['objects']if o['role']=='source')
                            self.assertEqual([g['point']for g in source['grips']],[g['point']for g in v['before']['objects'][0]['grips']])
                        source=next(o for o in v['after_script']['objects']if o['role']=='source')
                        self.assertEqual([g['index']for g in source['grips']if g['selected']],op['selected']if op.get('finish')=='Cancel'else [])
    def test_invalid_recipes_are_rejected_before_launch(self):
        for make,patches in [(grips,[dict(selected=[-1]),dict(copy=1),dict(surface='Sphere'),dict(edit=True,source='mesh'),dict(targets=[[0.,0.]]),dict(targets=[[2.,float('nan')]]),dict(extra=1)]),(reference,[dict(mode='Other'),dict(plane='Other'),dict(extra=1)])]:
            for patch in patches:
                with self.subTest(patch=patch),self.assertRaises(ValueError):validate_operations(dict(operations=[dict(copy.deepcopy(make()['operations'][0]),**patch)]))
