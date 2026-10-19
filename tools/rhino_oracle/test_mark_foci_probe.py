"""Bounded public commands, exact saved sources, and standalone worker staging."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
from unittest import TestCase
from .generate_mark_foci import request
from .mark_foci_probe import validate
from .structural_sdk import stage_helpers
HERE=Path(__file__).parent
class MarkFociProbe(TestCase):
    def test_bounded_recipes_and_standalone_staging(self):
        q=request();self.assertEqual(len(q['operations']),58)
        with tempfile.TemporaryDirectory()as d:
            stage_helpers(q,HERE,Path(d))
            self.assertEqual({p.name for p in Path(d).iterdir()},{'mark_foci_probe.py','join_probe.py','merge_edges_probe.py'})
            for p in Path(d).iterdir():self.assertEqual(p.read_bytes(),(HERE/p.name).read_bytes())
    def test_bad_recipes_reject_before_launch(self):
        base=request()['operations'][0]
        for patch in [dict(draw_axis=1),dict(preselect='yes'),dict(gauge=0.),dict(gauge=float('nan')),dict(elevate=13),dict(elevate=True),
                      dict(domain=[1.,1.]),dict(id='../invalid'),dict(family='surface'),dict(artifact_path='/tmp/file.json'),dict(extra=True)]:
            op=copy.deepcopy(base);op.update(patch)
            with self.subTest(patch=patch),self.assertRaises(ValueError):validate(op)
    def test_recorded_commands_sources_and_provenance(self):
        r=json.loads((HERE/'observations/mark_foci.json').read_text());q=request()
        self.assertEqual([v['id']for v in r['results']],[o['id']for o in q['operations']])
        self.assertIn('( DrawAxis=Yes ): _DrawAxis=_No',r['results'][0]['value']['history'])
        for row,op in zip(r['results'],q['operations']):
            actual=dict(row['value']['recipe']);expected=dict(op)
            self.assertEqual(Path(actual.pop('artifact_path')).name,Path(expected.pop('artifact_path')).name)
            self.assertEqual(actual,expected)
            self.assertEqual([e['name']for e in row['value']['events']].count('MarkFoci'),1)
            p=Path(op['artifact_path']);self.assertEqual(hashlib.sha256(p.read_bytes()).hexdigest(),r['provenance']['fixtures'][p.name])
        for name,digest in r['provenance']['sources'].items():
            if name in ('rhino_worker.py','structural_sdk.py'):continue
            self.assertEqual(hashlib.sha256((HERE/name).read_bytes()).hexdigest(),digest,name)
