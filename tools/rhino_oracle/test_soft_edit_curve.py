"""Immutable native evidence, complete recipes and isolated helper staging."""
import copy
import gzip
import hashlib
import json
import tempfile
from pathlib import Path
from unittest import TestCase
from .generate_soft_edit_curve import request
from .soft_edit_curve_probe import validate
from .structural_sdk import stage_helpers, validate_operations
from .generate_soft_edit_sequence import request as sequence_request

HERE=Path(__file__).parent


class SoftEditCurveTests(TestCase):
    def test_precise_sdk_diagnostics_and_repeated_getters(self):
        digest=lambda b:hashlib.sha256(b).hexdigest()
        for stem,count,folder in [('soft_edit_curve_lengths',187,'soft_edit_curve_command'),('soft_edit_curve_sequence',32,'soft_edit_curve_sequence')]:
            p=HERE/'fixtures'/(stem+'.json');q=json.loads(p.read_text());r=json.loads((HERE/'observations'/(stem+'.json')).read_text())
            self.assertEqual(len(r['results']),count)
            self.assertEqual([o['id']for o in q['operations']],[o['id']for o in r['results']])
            self.assertEqual(digest(p.read_bytes()),r['provenance']['request_sha256']);validate_operations(q)
            for name,want in r['provenance']['sources'].items():
                b=(HERE/name).read_bytes()
                if digest(b)!=want:b=gzip.decompress((HERE/'observations/source_snapshots'/(want+'.gz')).read_bytes())
                self.assertEqual(digest(b),want,name)
            for name,want in r['provenance']['fixtures'].items():
                self.assertEqual(digest((HERE/'fixtures'/folder/name).read_bytes()),want,name)
            for row in r['results']:
                self.assertNotIn('error',row);v=row['value']
                if stem=='soft_edit_curve_lengths':
                    self.assertTrue(v['snapped_projection_success'])
                    for sample in v['lengths']:
                        self.assertEqual(sample['default'],sample['explicit_1e8'])
                        self.assertGreaterEqual(sample['accurate'],0.)
                else:
                    self.assertNotIn('Unknown command',v['history']);self.assertEqual(len(v['events']),1)
                    self.assertEqual(len(v['after']),4 if v['recipe'].get('copy')else 1)
                    self.assertEqual(len(v['undo']),1)
                    self.assertEqual(len(v['redo']),len(v['after']))
        q=sequence_request()
        with tempfile.TemporaryDirectory()as d:
            stage_helpers(q,HERE,Path(d))
            self.assertEqual({p.name for p in Path(d).iterdir()},
                {'soft_edit_sequence_probe.py','soft_edit_curve_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py'})

    def test_native_sources_fixtures_and_command_completion(self):
        digest=lambda b:hashlib.sha256(b).hexdigest()
        path=HERE/'fixtures/soft_edit_curve_command.json'
        q=json.loads(path.read_text());r=json.loads((HERE/'observations/soft_edit_curve_command.json').read_text())
        self.assertEqual(len(q['operations']),187)
        self.assertEqual([o['id']for o in q['operations']],[o['id']for o in r['results']])
        self.assertEqual(digest(path.read_bytes()),r['provenance']['request_sha256'])
        validate_operations(q)
        for name,want in r['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if digest(data)!=want:data=gzip.decompress((HERE/'observations/source_snapshots'/(want+'.gz')).read_bytes())
            self.assertEqual(digest(data),want,name)
        for name,want in r['provenance']['fixtures'].items():
            self.assertEqual(digest((HERE/'fixtures/soft_edit_curve_command'/name).read_bytes()),want,name)
        for op,row in zip(q['operations'],r['results']):
            self.assertNotIn('error',row)
            v=row['value'];self.assertEqual(v['recipe'],op)
            completed=[e for e in v['events']if e['name']=='SoftEditCrv']
            self.assertEqual(len(completed),1,op['id'])
            self.assertNotIn('Unknown command',v['history'],op['id'])
            self.assertIn('after',v);self.assertIn('undo',v);self.assertIn('redo',v)
        generated=request()
        for a,b in zip(generated['operations'],q['operations']):
            self.assertEqual(Path(a.pop('artifact_path')).name,Path(b.pop('artifact_path')).name)
        self.assertEqual(generated,q)

    def test_closed_recipes_and_standalone_staging(self):
        q=request()
        with tempfile.TemporaryDirectory()as folder:
            stage_helpers(q,HERE,Path(folder))
            self.assertEqual({p.name for p in Path(folder).iterdir()},
                {'soft_edit_curve_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py'})
            for p in Path(folder).iterdir():self.assertEqual(p.read_bytes(),(HERE/p.name).read_bytes())
        base=q['operations'][0]
        for patch in [dict(distance=True),dict(distance=0.),dict(distance=float('nan')),dict(offset=[0.,float('inf'),0.]),
                dict(offset=['0 _Delete',0.,0.]),dict(stations=[]),dict(copy=1),dict(family='surface'),dict(extra=True)]:
            with self.subTest(patch=patch),self.assertRaises(ValueError):validate(dict(copy.deepcopy(base),**patch))
