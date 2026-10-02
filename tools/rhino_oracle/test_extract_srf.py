"""Face extraction captures require owned sources and complete observations."""
import copy
import json
from pathlib import Path
import unittest
from unittest.mock import Mock

from .client import _owned_artifact_request
from .extract_srf_cases import request,picking_request
from .extract_srf_probe import validate
from .extract_srf_capture import capture,validate_request
from .extract_srf_replay import canonical_response


class ExtractSrfTests(unittest.TestCase):
    def test_saved_sources_are_independent_and_shared_paths_are_temporary(self):
        saved=json.loads(Path(__file__).with_name('fixtures').joinpath('extract_srf_faces.json').read_text())
        self.assertEqual(saved,request())
        with _owned_artifact_request(saved) as prepared:
            validate_request(prepared)
            self.assertTrue(all('artifact_path' in s['brep'] for op in prepared['operations'] for s in op['sources']))
        self.assertNotIn('artifact_path',json.dumps(saved))

    def test_saved_mouse_inputs_are_independent_and_have_no_injected_selection_targets(self):
        saved=json.loads(Path(__file__).with_name('fixtures').joinpath('extract_srf_picking.json').read_text())
        self.assertEqual(saved,picking_request())
        self.assertEqual(len(saved['operations']),32)
        with _owned_artifact_request(saved) as prepared:validate_request(prepared)
        self.assertTrue(all(not op['components'] and op['pick']=='sequence' for op in saved['operations']))
        self.assertNotIn('artifact_path',json.dumps(saved))

    def test_invalid_indices_options_and_unowned_sources_are_rejected_before_launch(self):
        with _owned_artifact_request(request()) as prepared:
            operation=prepared['operations'][0]
            for key,value in [('components',[]),('components',[[0,-1]]),('components',[[0,True]]),
                    ('components',[[1,0]]),('copy',1),('output_current','Yes'),('source_layer',1),
                    ('undo_redo',0),('id','unsafe path'),('sources',[dict(brep={})])]:
                invalid=copy.deepcopy(operation);invalid[key]=value
                with self.subTest(key=key,value=value),self.assertRaises(ValueError):validate(invalid)
            invalid=copy.deepcopy(prepared);invalid['iterations']=2
            with self.assertRaises(ValueError):validate_request(invalid)
            invalid=copy.deepcopy(prepared);invalid['operations'][1]['id']=invalid['operations'][0]['id']
            with self.assertRaises(ValueError):validate_request(invalid)

    def test_incomplete_exports_never_reach_rhino(self):
        client=Mock();client.run_viboceros.return_value=dict(protocol_version=1,engine='viboceros',iterations=1,results=[])
        with self.assertRaisesRegex(ValueError,'incomplete independent'):capture(request(),client)
        client.run_rhino.assert_not_called()

    def test_mouse_sequences_require_owned_faces_bounded_steps_and_explicit_finish(self):
        with _owned_artifact_request(request()) as prepared:
            operation=prepared['operations'][0]
            operation.update(components=[],pick='sequence',steps=[dict(kind='click',component=[0,0],modifiers='plain')],finish='Enter')
            validate(operation)
            for key,value in [('components',[[0,0]]),('pick','unsafe'),('steps',[]),
                    ('steps',operation['steps']*65),('finish','Delete'),('finish',None)]:
                invalid=copy.deepcopy(operation);invalid[key]=value
                with self.subTest(key=key,value=value),self.assertRaises(ValueError):validate(invalid)
            for key,value in [('component',[0,True]),('component',[0,-1]),('component',[1,0]),
                    ('kind','window'),('modifiers','alt'),('extra',True)]:
                invalid=copy.deepcopy(operation);invalid['steps'][0][key]=value
                with self.subTest(key=key,value=value),self.assertRaises(ValueError):validate(invalid)
            for step in [dict(kind='key',value='Copy=Yes'),dict(kind='key',value='None'),
                    dict(kind='window',corners=[[0,0,0],[10,10,0]],modifiers='plain')]:
                valid=copy.deepcopy(operation);valid['steps']=[step];validate(valid)
            for step in [dict(kind='key',value='Delete'),dict(kind='key',value='Copy=Maybe'),
                    dict(kind='window',corners=[[False,0,0],[10,10,0]],modifiers='plain'),
                    dict(kind='window',corners=[[0,0,0],[10,float('nan'),0]],modifiers='plain'),
                    dict(kind='window',corners=[[0,0,0],[10,float('inf'),0]],modifiers='plain')]:
                invalid=copy.deepcopy(operation);invalid['steps']=[step]
                with self.subTest(step=step),self.assertRaises(ValueError):validate(invalid)
            invalid=copy.deepcopy(operation);invalid['steps']=[dict(kind='key',value='None')]+invalid['steps']
            with self.assertRaises(ValueError):validate(invalid)

    def test_replay_preserves_output_order_and_every_topology_index(self):
        req=dict(operations=[dict(id='case')])
        value=dict(after=[dict(source=None,edges=[3,0]),dict(source=0,edges=[2,1])],history='diagnostic')
        response=dict(protocol_version=1,iterations=1,results=[dict(id='case',value=value)])
        result=canonical_response(req,response)
        self.assertEqual(result['results'][0]['value']['after'],value['after'])
        self.assertIn('history',value)


if __name__=='__main__':unittest.main()
