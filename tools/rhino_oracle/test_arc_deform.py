"""Native Arc approximation receipts and immutable qualification of raw rows."""
import json
from pathlib import Path
from unittest import TestCase
from .arc_command_probe import validate
from .arc_tangent_probe import validate as tangent_validate
BASE=Path(__file__).parent
class ArcDeformTests(TestCase):
    def test_qualified_public_getters_geometry_history_and_saved_settings(self):
        successes=cancels=0
        for profile,count in [('basic',7),('ordered',10),('constructions',23),('settings',9),('cursor',12),('tangent',7),('midpoint_qualified',3)]:
            q=json.loads((BASE/f'fixtures/arc_deform_{profile}.json').read_text())
            n=json.loads((BASE/f'observations/arc_deform_{profile}_rhino_20261017.json').read_text())
            self.assertEqual(len(q['operations']),count)
            self.assertEqual(len(n['results']),count)
            for op,row in zip(q['operations'],n['results']):
                (tangent_validate if op['op']=='arc_tangent' else validate)(op);self.assertEqual(op['id'],row['id']);v=row['value'];t=v['input_trace']
                self.assertEqual(t['errors'],[]);self.assertEqual(t['completed'],len(op['actions']))
                self.assertNotIn('Unknown command',v['history'])
                self.assertEqual(v['before']['outputs'],v['undo']['outputs'])
                self.assertEqual(v['after_script']['outputs'],v['redo']['outputs'])
                output=bool(v['after_script']['outputs']);successes+=output;cancels+=not output
                self.assertEqual([e['result']for e in v['events']if e['name']=='Arc'],['Success'if output else'Cancel'])
                if op.get('record_preferences'):
                    trace=v['preference_input_trace'];self.assertEqual(trace['errors'],[]);self.assertEqual(trace['completed'],2)
                    self.assertIn('Degree=',trace['prompts'][1]);self.assertIn('PointCount=',trace['prompts'][1])
        self.assertEqual((successes,cancels),(68,3))
    def test_qualified_rows_preserve_original_native_capture(self):
        for folder,key,suffix in [('fixtures','operations',''),('observations','results','_rhino_20261017')]:
            def rows(name):return json.loads((BASE/folder/f'arc_deform_{name}{suffix}.json').read_text())[key]
            for raw,target,indices in [('initial','basic',[0,2,4,6,8,9,10]),('boundaries','ordered',[0,1,2,3,5,6,8,9,10,11]),('modes','constructions',[0,1,2,3]+list(range(6,25))),('preferences','settings',[0,1,2,4,5,6,7,8,9])]:
                source=rows(raw);self.assertEqual(rows(target),[source[i]for i in indices])
            self.assertEqual(rows('midpoint_qualified'),rows('midpoint_endings')[:1]+rows('midpoint_getters'))
        raw=json.loads((BASE/'observations/arc_deform_boundaries_rhino_20261017.json').read_text())
        for i in [4,7]:
            v=raw['results'][i]['value'];self.assertEqual(v['after_script']['outputs'],[])
            self.assertEqual([e['result']for e in v['events']if e['name']=='Arc'],['Success'])
        raw=json.loads((BASE/'observations/arc_deform_preferences_rhino_20261017.json').read_text())
        v=raw['results'][3]['value'];self.assertEqual(v['input_trace']['errors'],['owned Arc input timeout'])
        self.assertEqual(v['input_trace']['prompts'][-4:],['PointCount <4>']*4)
    def test_preference_witness_flag_is_strict_boolean(self):
        op=json.loads((BASE/'fixtures/arc_deform_settings.json').read_text())['operations'][2]
        for value in [1,'yes',[],None]:
            with self.assertRaises(ValueError):validate(dict(op,record_preferences=value))
