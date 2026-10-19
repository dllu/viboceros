"""Bounded public commands, independent saved sources, and capture integrity."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
from unittest import TestCase
from .generate_insert_kink import request
from .insert_kink_probe import validate
from .structural_sdk import stage_helpers
HERE=Path(__file__).parent
class InsertKinkProbe(TestCase):
    def test_recipes_and_standalone_staging(self):
        q=request();self.assertEqual(len(q['operations']),101)
        with tempfile.TemporaryDirectory()as d:
            stage_helpers(q,HERE,Path(d))
            self.assertEqual({p.name for p in Path(d).iterdir()},{'insert_kink_probe.py','join_probe.py','merge_edges_probe.py'})
            for p in Path(d).iterdir():self.assertEqual(p.read_bytes(),(HERE/p.name).read_bytes())
    def test_bad_recipes_reject_before_launch(self):
        base=request()['operations'][0]
        for patch in [dict(stations=[True]),dict(stations=[float('nan')]),dict(stations=[-1]),dict(stations=[0]*17),dict(preselect=1),dict(midpoints=1),dict(gauge=0.),dict(gauge=1e300),dict(elevate=13),dict(face=6),dict(direction='bad'),dict(domain=[0.,0.]),dict(subcurve=[.2,.8]),dict(refine=float('nan')),dict(id='../bad'),dict(extra=True)]:
            op=copy.deepcopy(base);op.update(patch)
            with self.subTest(patch=patch),self.assertRaises(ValueError):validate(op)
    def test_capture_geometry_files_and_provenance(self):
        capture=json.loads((HERE/'observations/insert_kink.json').read_text());q=request()
        self.assertEqual([r['id']for r in capture['results']],[o['id']for o in q['operations']])
        for row,op in zip(capture['results'],q['operations']):
            actual=dict(row['value']['recipe']);expected=dict(op)
            self.assertEqual(Path(actual.pop('artifact_path')).name,Path(expected.pop('artifact_path')).name)
            self.assertEqual(actual,expected)
            self.assertEqual([e['name']for e in row['value']['events']].count('InsertKink'),1)
            path=Path(op['artifact_path']);self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),capture['provenance']['fixtures'][path.name])
        for name,digest in capture['provenance']['sources'].items():self.assertEqual(hashlib.sha256((HERE/name).read_bytes()).hexdigest(),digest,name)
