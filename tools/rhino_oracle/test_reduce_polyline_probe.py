"""Bounds, standalone staging, and native source provenance for reduction."""
import hashlib
import json
from pathlib import Path
import tempfile
from unittest import TestCase
from .generate_reduce_polyline import request
from .reduce_polyline_probe import validate
from .structural_sdk import stage_helpers, validate_operations

HERE=Path(__file__).parent

class ReducePolylineProbe(TestCase):
    def recipe(self,**kw):
        op=dict(op='reduce_polyline_command',id='test',points=[[0.,0.,0.],[1.,.1,0.],[2.,0.,0.]],form='polyline',
                reduction_tolerance=.15,artifact_path='/tmp/source.3dm')
        op.update(kw);return op

    def test_bounded_matrix_and_real_worker_helpers(self):
        q=request();self.assertGreaterEqual(len(q['operations']),55);validate_operations(q)
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(q,HERE,Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()},{'reduce_polyline_probe.py','join_probe.py','merge_edges_probe.py'})
            for p in Path(directory).iterdir():self.assertEqual(p.read_bytes(),(HERE/p.name).read_bytes())

    def test_invalid_or_ambiguous_recipes_reject_before_launch(self):
        for kw in [dict(reduction_tolerance=v) for v in (0.,-1.,True,float('nan'),float('inf'),1e7)]+[
            dict(points=[]),dict(points=[[0.,0.,0.]]*3),dict(points=[[0.,False,0.],[1.,0.,0.]]),
            dict(points=[[0.,0.,0.],[float('inf'),0.,0.]]),dict(points=[[0.,0.,0.],[1e13,0.,0.]]),
            dict(copy=1),dict(preselect=1),dict(benchmark=1),dict(entry='option'),dict(form='surface'),
            dict(form='line'),dict(form='cubic'),dict(domain=[1.,1.]),dict(domain=[0.,float('nan')]),
            dict(extra=True),dict(id='../bad'),dict(artifact_path='/tmp/source.json')]:
            with self.subTest(kw=kw),self.assertRaises(ValueError):validate(self.recipe(**kw))

    def test_current_capture_has_real_commands_and_exact_source_receipts(self):
        r=json.loads((HERE/'observations/reduce_polyline.json').read_text());q=request()
        self.assertEqual([v['id']for v in r['results']],[o['id']for o in q['operations']])
        for row,op in zip(r['results'],q['operations']):
            actual=dict(row['value']['recipe']);expected=dict(op)
            self.assertEqual(Path(actual.pop('artifact_path')).name,Path(expected.pop('artifact_path')).name)
            self.assertEqual(actual,expected)
            self.assertEqual([e['name']for e in row['value']['events']].count('ReducePolyline'),1)
            p=Path(op['artifact_path']);self.assertEqual(hashlib.sha256(p.read_bytes()).hexdigest(),r['provenance']['fixtures'][p.name])
        for name,digest in r['provenance']['sources'].items():
            if name in ('rhino_worker.py','structural_sdk.py'):
                continue # Shared dispatchers can gain unrelated probes later.
            self.assertEqual(hashlib.sha256((HERE/name).read_bytes()).hexdigest(),digest,name)

    def test_vendored_public_reference_is_pinned_and_unmodified(self):
        folder=HERE.parents[1]/'third_party/rhino3dm-polyline'
        p=json.loads((folder/'provenance.json').read_text())
        self.assertEqual(len(p['revision']),40)
        for name,digest in p['files'].items():
            self.assertEqual(hashlib.sha256((folder/name).read_bytes()).hexdigest(),digest)
