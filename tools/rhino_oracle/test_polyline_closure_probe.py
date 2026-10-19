"""Bounded closure recipes, standalone staging, and native source receipts."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
from unittest import TestCase
from .generate_polyline_closure import request
from .polyline_closure_probe import validate
from .structural_sdk import stage_helpers
HERE=Path(__file__).parent
class PolylineClosureProbe(TestCase):
    def test_bounds_and_staging(self):
        q=request();self.assertLessEqual(len(q['operations'][0]['cases']),256)
        with tempfile.TemporaryDirectory()as d:
            stage_helpers(q,HERE,Path(d))
            self.assertEqual({p.name for p in Path(d).iterdir()},{'polyline_closure_probe.py','join_probe.py','merge_edges_probe.py'})
            for p in Path(d).iterdir():self.assertEqual(p.read_bytes(),(HERE/p.name).read_bytes())
    def test_invalid_requests_fail_before_launch(self):
        base=request()['operations'][0]
        for patch in [dict(points=[]),dict(points=[[0.,0.,0.]]*4),dict(form='surface'),dict(domain=[1.,1.]),dict(degree=True),dict(grip_index=-1),dict(move_grip=1),dict(move_vector=[float('nan'),0.,0.]),dict(extra=True)]:
            op=copy.deepcopy(base);op['cases'][0].update(patch)
            with self.subTest(patch=patch),self.assertRaises(ValueError):validate(op)
    def test_recorded_matrix_and_exact_sources(self):
        q=request();r=json.loads((HERE/'observations/polyline_closure.json').read_text());op=q['operations'][0];v=r['results'][0]['value']
        self.assertEqual([c['recipe']for c in v['rows']],op['cases'])
        for row in v['rows']:
            if row['valid']:self.assertIsInstance(row['closed'],bool)
            if 'extract'in row:self.assertEqual([e['name']for e in row['extract']['events']].count('ExtractPt'),1)
            if 'move_grip'in row:self.assertEqual([e['name']for e in row['move_grip']['events']].count('Move'),1)
        for name,query in v['queries'].items():self.assertEqual([e['name']for e in query['events']].count(name),1)
        self.assertEqual(hashlib.sha256(Path(op['artifact_path']).read_bytes()).hexdigest(),r['provenance']['fixture'])
        for name,digest in r['provenance']['sources'].items():
            if name in ('rhino_worker.py','structural_sdk.py'):continue
            self.assertEqual(hashlib.sha256((HERE/name).read_bytes()).hexdigest(),digest,name)
