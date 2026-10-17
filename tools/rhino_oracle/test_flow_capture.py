import copy,json,math
from pathlib import Path
from unittest import TestCase
from .flow_probe import validate
BASE=Path(__file__).parent

class FlowCaptureTests(TestCase):
    def test_authored_flow_recipes_and_bounded_inputs(self):
        for name in ('flow','flow_frames'):
            request=json.loads((BASE/'fixtures'/(name+'.json')).read_text())
            for op in request['operations']:validate(op)
        original=json.loads((BASE/'fixtures/flow.json').read_text())['operations'][0]
        for key,value in [('id','bad\nPICK'),('points',[[0,0,0]]),('points',[[0,0,0]]*2049),('points',[[0,0,0]]*3+[[0,True,0]]),('points',[[0,0,0]]*3+[[0,float('inf'),0]]),('options',{'stretch':1}),('options',{'unsupported':True}),('command','yes'),('diagnostics',1)]:
            op=copy.deepcopy(original);op[key]=value
            with self.assertRaises(ValueError,msg=key):validate(op)

    def test_native_sdk_witnesses_and_four_actual_commands_are_retained(self):
        commands=0
        for name,count in [('flow',44),('flow_frames',18)]:
            request=json.loads((BASE/'fixtures'/(name+'.json')).read_text())
            native=json.loads((BASE/'observations'/(name+'_rhino_20261016.json')).read_text())
            self.assertEqual(native['engine'],'rhino')
            self.assertEqual(len(native['results']),count)
            self.assertEqual(len({r['id']for r in native['results']}),count)
            for op,row in zip(request['operations'],native['results']):
                self.assertEqual(op['id'],row['id']);v=row['value']
                self.assertGreater(v['base_length'],0);self.assertGreater(v['target_length'],0)
                self.assertEqual(len(v['sdk_points']),len(op['points']))
                self.assertTrue(all(len(p)==3 and all(math.isfinite(x)for x in p)for p in v['sdk_points']))
                c=v['command']
                if not op.get('command'):
                    self.assertIsNone(c);continue
                commands+=1
                self.assertTrue(c['returned']);self.assertTrue(c['guides_unchanged'])
                self.assertNotIn('Unknown command',c['history'])
                self.assertEqual(c['guides_selected'],[False,False]);self.assertEqual(c['group_count'],1)
                self.assertEqual(len(c['outputs']),1);cloud=c['outputs'][0]
                self.assertTrue(cloud['original']);self.assertTrue(cloud['selected']);self.assertEqual(cloud['group_count'],1)
                self.assertEqual(len(cloud['points']),len(op['points']))
                self.assertLessEqual(max(abs(a-b)for p,q in zip(cloud['points'],v['sdk_points'])for a,b in zip(p,q)),1e-12)
        self.assertEqual(commands,4)

    def test_flow_command_and_mixed_plane_captures_have_clean_macros(self):
        commands=0
        for name,count in [('flow_command',24),('flow_planes',16),('flow_equivalence',19),('flow_precision',19)]:
            request=json.loads((BASE/'fixtures'/(name+'.json')).read_text())
            native=json.loads((BASE/'observations'/(name+'_rhino_20261016.json')).read_text())
            self.assertEqual(len(request['operations']),count)
            self.assertEqual(len(native['results']),count)
            for op,row in zip(request['operations'],native['results']):
                validate(op)
                self.assertEqual(op['id'],row['id'])
                self.assertEqual(len(op['points']),len(row['value']['sdk_points']))
                self.assertTrue(all(math.isfinite(x)for p in row['value']['sdk_points']for x in p))
                if not op.get('command'):continue
                commands+=1;c=row['value']['command']
                self.assertTrue(c['returned']);self.assertTrue(c['guides_unchanged'])
                self.assertNotIn('Unknown command',c['history'])
                self.assertEqual(c['guides_selected'],[False,False])
                copy=op.get('options',{}).get('copy',False)
                self.assertEqual(c['group_count'],2 if copy else 1)
                self.assertEqual(len(c['outputs']),2 if copy else 1)
                original=next(o for o in c['outputs']if o['original'])
                self.assertTrue(original['selected'])
                if copy:self.assertFalse(next(o for o in c['outputs']if not o['original'])['selected'])
        self.assertEqual(commands,40)
