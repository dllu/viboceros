"""Guided reference and true command completion evidence, including provenance."""
import gzip,hashlib,json,tempfile
from pathlib import Path
from unittest import TestCase
from .generate_splop_guided import request as guided
from .generate_splop_finish import request as finish
from .structural_sdk import validate_operations,stage_helpers
HERE=Path(__file__).resolve().parent
class SplopGuidedTests(TestCase):
    def test_requests_captures_sources_and_transitive_staging(self):
        for name,make,count in [('splop_guided',guided,317),('splop_finish',finish,48)]:
            q=json.loads((HERE/'fixtures'/f'{name}.json').read_text());r=json.loads((HERE/'observations'/f'{name}.json').read_text())
            self.assertEqual(q,make());self.assertEqual(len(r['results']),count)
            self.assertEqual(hashlib.sha256((HERE/'fixtures'/f'{name}.json').read_bytes()).hexdigest(),r['provenance']['request_sha256'])
            for source,sha in r['provenance']['sources'].items():
                data=(HERE/source).read_bytes()
                if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/f'{sha}.gz').read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),sha,source)
            with tempfile.TemporaryDirectory() as folder:
                stage_helpers(q,HERE,Path(folder));self.assertTrue((Path(folder)/f'{name}_probe.py').exists())
                for p in Path(folder).iterdir():self.assertEqual(p.read_bytes(),(HERE/p.name).read_bytes())
    def test_guided_command_lifecycle_and_real_history(self):
        q=guided();r=json.loads((HERE/'observations/splop_guided.json').read_text())
        for op,row in zip(q['operations'],r['results']):
            with self.subTest(id=op['id']):
                self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row);v=row['value']
                cancelled=op.get('finish')=='CancelReference';self.assertEqual(v['success'],not cancelled)
                for phase in ['after_script','undo','redo']:
                    self.assertFalse(v[phase]['target_selected'])
                    for obj in v[phase]['objects']:
                        if obj['role']=='guide':self.assertFalse(obj['selected']);self.assertTrue(all(not g['selected']for g in obj['grips']))
                if cancelled:
                    self.assertIn('Nothing to undo',v['undo_history']);self.assertEqual(v['after_script'],v['undo']);self.assertEqual(v['undo'],v['redo'])
                else:self.assertIn('Undoing Splop',v['undo_history']);self.assertIn('Redoing Splop',v['undo_history'])
    def test_enter_and_escape_preserve_preselected_source_picks(self):
        q=finish();r=json.loads((HERE/'observations/splop_finish.json').read_text())
        for op,row in zip(q['operations'],r['results']):
            with self.subTest(id=op['id']):
                self.assertNotIn('error',row);v=row['value'];self.assertEqual(v['success'],op['finish']=='Enter')
                self.assertFalse(v['executed_macro'].endswith(' _Enter _Cancel'))
                self.assertIn('Undoing Splop',v['undo_history']);self.assertIn('Redoing Splop',v['undo_history'])
                for phase in ['after_script','undo','redo']:
                    source=next(o for o in v[phase]['objects']if o.get('role')=='source' or o.get('source')==0)
                    self.assertEqual(source['selected'],not op['postselect']if op['kind']=='Whole'else op['parent'])
                    if op['kind']=='Grips':self.assertEqual([g['index']for g in source['grips']if g['selected']],[0,1])
    def test_invalid_guides_and_finish_recipes_are_rejected_without_launch(self):
        for make,patches in [(guided,[dict(mode='Other'),dict(kind='Other'),dict(surface='Other'),dict(source='Cloud'),dict(copy=1),dict(extra=True)]),(finish,[dict(kind='Other'),dict(finish='IdleCancel'),dict(copy=True),dict(edit=1)])]:
            for patch in patches:
                with self.subTest(patch=patch),self.assertRaises(ValueError):validate_operations(dict(operations=[dict(make()['operations'][0],**patch)]))
    def test_input_diagnostic_is_retained_as_a_separate_macro(self):
        d=json.loads(gzip.decompress((HERE.parent.parent/'docs/splop-guided-input-diagnostic.json.gz').read_bytes()))
        self.assertEqual(len(d['response']['results']),10)
        self.assertFalse(d['response']['results'][6]['value']['success'])
        for name,sha in d['response']['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/f'{sha}.gz').read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha)
