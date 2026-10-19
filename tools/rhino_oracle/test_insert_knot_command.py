"""Bounded public command recipes and retained geometry/source provenance."""
import copy,hashlib,json,tempfile,gzip
from pathlib import Path
from unittest import TestCase
from .generate_insert_knot_command import request
from .insert_knot_command_probe import validate
from .structural_sdk import stage_helpers
HERE=Path(__file__).parent
def captured_source_digest(name,digest):
    data=(HERE/name).read_bytes()
    if name in('rhino_worker.py','structural_sdk.py')and hashlib.sha256(data).hexdigest()!=digest:
        data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
    return hashlib.sha256(data).hexdigest()
class InsertKnotCommands(TestCase):
    def test_symmetric_pairs_centers_and_parameters(self):
        from .generate_insert_knot_symmetry import request as symmetry
        q=symmetry();capture=json.loads((HERE/'observations/insert_knot_symmetry.json').read_text());self.assertEqual(len(q['operations']),19)
        self.assertEqual([o['id']for o in q['operations']],[r['id']for r in capture['results']])
        for op,row in zip(q['operations'],capture['results']):
            v=row['value'];self.assertEqual(v['recipe'],op);self.assertTrue(v['success']);self.assertEqual([e['name']for e in v['events']].count('InsertKnot'),1)
            path=Path(op['artifact_path']);self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),capture['provenance']['fixtures'][path.name])
        for name,digest in capture['provenance']['sources'].items():
            self.assertEqual(captured_source_digest(name,digest),digest,name)
    def test_followup_actions_fixtures_and_sources(self):
        from .generate_insert_knot_followup import request as followup
        q=followup();capture=json.loads((HERE/'observations/insert_knot_followup.json').read_text());self.assertEqual(len(q['operations']),49)
        self.assertEqual([o['id']for o in q['operations']],[r['id']for r in capture['results']])
        for op,row in zip(q['operations'],capture['results']):
            v=row['value'];self.assertEqual(v['recipe'],op);events=[e for e in v['events']if e['name']=='InsertKnot'];self.assertEqual(len(events),1)
            self.assertEqual(v['success'],events[0]['result']=='Success');path=Path(op['artifact_path']);self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),capture['provenance']['fixtures'][path.name])
        for name,digest in capture['provenance']['sources'].items():
            self.assertEqual(captured_source_digest(name,digest),digest,name)
    def test_bounded_request_and_helpers(self):
        q=request();self.assertEqual(len(q['operations']),156)
        self.assertEqual(len({o['id']for o in q['operations']}),156)
        with tempfile.TemporaryDirectory()as d:
            stage_helpers(q,HERE,Path(d))
            self.assertEqual({p.name for p in Path(d).iterdir()},{'insert_knot_command_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py'})
            for p in Path(d).iterdir():self.assertEqual(p.read_bytes(),(HERE/p.name).read_bytes())
    def test_malformed_actions_rejected(self):
        base=request()['operations'][0]
        for actions in [None,[True],['!'],['Undo _Delete'],[dict(point=[])],[dict(point=[.1,.2,.3])],[dict(point=float('nan'))],[dict(point=True)],[dict(parameter=float('inf'))],[dict(parameter=True)],[dict(parameter=1,point=.2)],['Automatic']*33]:
            op=copy.deepcopy(base);op['actions']=actions
            with self.subTest(actions=actions),self.assertRaises(ValueError):validate(op)
    def test_native_commands_sources_and_outcomes(self):
        capture=json.loads((HERE/'observations/insert_knot_command.json').read_text());q=request()
        self.assertEqual([r['id']for r in capture['results']],[o['id']for o in q['operations']])
        results={}
        for row,op in zip(capture['results'],q['operations']):
            v=row['value'];actual=dict(v['recipe']);expected=dict(op)
            self.assertEqual(Path(actual.pop('artifact_path')).name,Path(expected.pop('artifact_path')).name);self.assertEqual(actual,expected)
            events=[e for e in v['events']if e['name']=='InsertKnot'];self.assertEqual(len(events),1)
            self.assertEqual(v['success'],events[0]['result']=='Success')
            results[events[0]['result']]=results.get(events[0]['result'],0)+1
            self.assertTrue(v['macro'].startswith('_InsertKnot '))
            self.assertEqual(len(v['picks']),sum(isinstance(a,dict)and'point'in a for a in op['actions']))
            original=next(o for o in v['after']if o['id']==v['source_id']);before=v['before'][0]
            for key in ['name','layer','groups','user_text']:self.assertEqual(original[key],before[key],(row['id'],key))
            path=Path(op['artifact_path']);self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),capture['provenance']['fixtures'][path.name])
        self.assertEqual(results,dict(Success=127,Cancel=28,Failure=1))
        for name,digest in capture['provenance']['sources'].items():
            self.assertEqual(captured_source_digest(name,digest),digest,name)
