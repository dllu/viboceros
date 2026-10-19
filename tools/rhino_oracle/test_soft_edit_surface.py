"""Surface command evidence is immutable and transitive producers stay staged."""
import copy
import gzip
import hashlib
import json
import tempfile
from pathlib import Path
from unittest import TestCase
from .generate_soft_edit_surface import request
from .generate_soft_edit_surface_shapes import request as shapes
from .generate_soft_edit_surface_sequence import request as sequences
from .soft_edit_surface_probe import validate
from .structural_sdk import stage_helpers,validate_operations
HERE=Path(__file__).parent
class SurfaceSoftEditTests(TestCase):
    def test_native_sources_fixtures_and_terminal_command_events(self):
        digest=lambda b:hashlib.sha256(b).hexdigest()
        for stem,count in [('soft_edit_surface_command',108),('soft_edit_surface_shapes',105),('soft_edit_surface_sequence',16)]:
            p=HERE/'fixtures'/(stem+'.json');q=json.loads(p.read_text());r=json.loads((HERE/'observations'/(stem+'.json')).read_text())
            self.assertEqual(len(r['results']),count);self.assertEqual([o['id']for o in q['operations']],[o['id']for o in r['results']]);validate_operations(q)
            self.assertEqual(digest(p.read_bytes()),r['provenance']['request_sha256'])
            for name,want in r['provenance']['sources'].items():
                b=(HERE/name).read_bytes()
                if digest(b)!=want:b=gzip.decompress((HERE/'observations/source_snapshots'/(want+'.gz')).read_bytes())
                self.assertEqual(digest(b),want,name)
            for name,want in r['provenance']['fixtures'].items():self.assertEqual(digest((HERE/'fixtures'/stem/name).read_bytes()),want,name)
            for op,row in zip(q['operations'],r['results']):
                self.assertNotIn('error',row);v=row['value'];self.assertEqual(v['recipe'],op);self.assertEqual([e['name']for e in v['events']],['SelID','SoftEditSrf']if op.get('preselect')is False else['SoftEditSrf']);self.assertNotIn('Unknown command',v['history']);self.assertIn('after',v);self.assertIn('undo',v);self.assertIn('redo',v)
    def test_reproducible_recipes_and_transitive_helper_staging(self):
        for stem,factory in [('soft_edit_surface_command',request),('soft_edit_surface_shapes',shapes),('soft_edit_surface_sequence',sequences)]:
            q=factory();expected=json.loads((HERE/'fixtures'/(stem+'.json')).read_text())
            for a,b in zip(q['operations'],expected['operations']):self.assertEqual(Path(a.pop('artifact_path')).name,Path(b.pop('artifact_path')).name)
            self.assertEqual(q,expected)
            with tempfile.TemporaryDirectory()as folder:
                stage_helpers(q,HERE,Path(folder));names={p.name for p in Path(folder).iterdir()}
                self.assertTrue({'soft_edit_surface_probe.py','soft_edit_curve_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py'}<=names)
                for p in Path(folder).iterdir():self.assertEqual(p.read_bytes(),(HERE/p.name).read_bytes())
    def test_invalid_recipes_fail_before_native_execution(self):
        base=request()['operations'][0]
        for patch in [dict(distance=[0.,1.]),dict(distance=[True,1.]),dict(distance=[float('nan'),1.]),dict(offset=[0.,float('inf'),0.]),dict(stations=[]),dict(copy=1),dict(constraint='Delete'),dict(family='cubic'),dict(extra=True)]:
            with self.subTest(patch=patch),self.assertRaises(ValueError):validate(dict(copy.deepcopy(base),**patch))
