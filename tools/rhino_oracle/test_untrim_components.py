import copy
import json
from pathlib import Path
import unittest
from unittest.mock import Mock

from .untrim_component_cases import request, partial_request, history_request, upper_request
from .untrim_component_capture import capture
from .untrim_component_probe import validate
from .client import OracleProtocolError
from .untrim_replay import replay

ROOT=Path(__file__).parent


class UntrimComponentTests(unittest.TestCase):
    def test_native_local_undo_removes_retained_objects_and_escape_keeps_edits(self):
        fixture=json.loads((ROOT/'fixtures/untrim_history.json').read_text())
        observed=json.loads((ROOT/'observations/untrim_history.json').read_text())
        self.assertEqual(history_request(),fixture);self.assertEqual(len(observed['results']),24)
        for op,row in zip(fixture['operations'],observed['results']):
            self.assertEqual(op['id'],row['id']);value=row['value']
            self.assertEqual(value['succeeded'],op['finish']=='Enter')
            self.assertEqual(value['undo_states'],[value['before']])
            self.assertEqual(len(value['input_states']),len(op['components']))
            if len(op['components'])==1:
                self.assertEqual(value['before'],value['after']);self.assertFalse(value['history_tested'])
            else:
                self.assertEqual(value['input_states'][0],value['input_states'][1]);self.assertEqual(value['after'],value['input_states'][1])
                self.assertEqual(value['undo'],value['before']);self.assertEqual(value['redo'],value['after'])
        client=Mock();client.run_viboceros.return_value=dict(copy.deepcopy(observed),engine='viboceros')
        self.assertTrue(replay(fixture,observed,client).passed)
        bad=copy.deepcopy(observed);bad['results'][0]['value']['undo_states'][0][0]['groups']=[]
        self.assertFalse(replay(fixture,bad,client).passed)
    def test_upper_opening_sources_and_local_undo_checkpoint_validation(self):
        self.assertEqual(upper_request(),json.loads((ROOT/'fixtures/untrim_upper.json').read_text()))
        op=history_request()['operations'][0];op['sources'][0]['brep']['artifact_path']='/owned/source.3dm';validate(op)
        for changes in [dict(undo_after=None),dict(undo_after=[True]),dict(undo_after=[0]),dict(undo_after=[2]),
                dict(undo_after=[1,1]),dict(pick='preselect',view=None)]:
            with self.subTest(changes=changes),self.assertRaises(ValueError):validate(dict(op,**changes))
    def test_partial_sources_regenerate_and_geometry_incidence_remains_in_replay(self):
        fixture=json.loads((ROOT/'fixtures/untrim_partial.json').read_text())
        observed=json.loads((ROOT/'observations/untrim_partial.json').read_text())
        self.assertEqual(partial_request(),fixture);self.assertEqual(len(observed['results']),6)
        client=Mock();client.run_viboceros.return_value=dict(copy.deepcopy(observed),engine='viboceros')
        self.assertTrue(replay(fixture,observed,client).passed)
        for field in ('incidence','weight','uv','input_state'):
            bad=copy.deepcopy(observed);value=bad['results'][0]['value'];g=value['after'][0]['geometry']['definition']
            if field=='incidence':g['topology']['edges'][0][0]=1
            elif field=='weight':g['edges'][0]['curve']['definition']['control_points'][0]['weight']=2.
            elif field=='uv':g['faces'][0]['loops'][0][0]['definition']['domain'][0]+=1.
            else:value['input_states'][0][0]['groups']=[]
            self.assertFalse(replay(fixture,bad,client).passed,field)
    def test_independent_sources_and_immediate_geometry_states_regenerate(self):
        fixture=json.loads((ROOT/'fixtures/untrim_components.json').read_text())
        observed=json.loads((ROOT/'observations/untrim_components.json').read_text())
        self.assertEqual(request(),fixture)
        self.assertEqual(len(observed['results']),71)
        for op,row in zip(fixture['operations'],observed['results']):
            self.assertEqual(op['id'],row['id']);value=row['value']
            self.assertEqual(value['component_selection']['after'],[])
            if op['pick']=='preselect':
                self.assertFalse(value['succeeded']);self.assertFalse(value['history_tested'])
                self.assertEqual(value['before'],value['after'])
                self.assertEqual(value['component_selection']['before'],[[s,'edge',i] for s,i in sorted(op['components'])])
            else:
                self.assertTrue(value['succeeded'])
                self.assertEqual(len(value['input_states']),len(op['components']))
                self.assertEqual(value['input_states'][-1],value['after'])
            if value['history_tested']:
                self.assertEqual(value['undo'],value['before']);self.assertEqual(value['redo'],value['after'])
                self.assertEqual(value['component_selection']['undo'],[]);self.assertEqual(value['component_selection']['redo'],[])

    def test_capture_validates_all_source_exports_and_removes_owned_artifacts(self):
        fixture=request();original=copy.deepcopy(fixture);client=Mock();paths=[]
        def export(prepared,timeout):
            for op in prepared['operations']:
                self.assertEqual(op['op'],'brep_remove_holes');self.assertEqual(op['loops'],[])
                path=Path(op['source']['artifact_path']);self.assertTrue(path.parent.is_dir());paths.append(path)
            return dict(protocol_version=1,engine='viboceros',iterations=1,results=[dict(id=op['id'],elapsed_ns=0,value={}) for op in prepared['operations']])
        def native(prepared,timeout):
            self.assertLessEqual(len(prepared['operations']),8)
            self.assertEqual(len(paths),71)
            return dict(protocol_version=1,engine='rhino',engine_version='test',iterations=1,
                results=[dict(id=op['id'],elapsed_ns=0,value={}) for op in prepared['operations']])
        client.run_viboceros.side_effect=export;client.run_rhino.side_effect=native
        observed=capture(fixture,client)
        self.assertEqual([row['id'] for row in observed['results']],[op['id'] for op in fixture['operations']]);self.assertEqual(fixture,original)
        self.assertEqual(client.run_rhino.call_count,9)
        self.assertEqual(len(paths),71);self.assertEqual(len(set(paths)),71)
        self.assertTrue(all(not path.parent.exists() for path in paths))
        client.run_viboceros.return_value={};client.run_viboceros.side_effect=None;client.reset_mock()
        with self.assertRaises(OracleProtocolError):capture(fixture,client)
        client.run_rhino.assert_not_called()

    def test_unbounded_indices_unowned_sources_and_command_injection_are_rejected(self):
        base=copy.deepcopy(request()['operations'][0]);base['sources'][0]['brep']['artifact_path']='/owned/source.3dm';validate(base)
        for changes in [dict(id='x _Delete'),dict(all_similar=1),dict(keep_trim_objects='Yes'),dict(undo_redo=1),
                dict(sources=[]),dict(sources=[{}]),dict(components=[[1,1]]),dict(components=[[0,True]]),
                dict(components=[[0,-1]]),dict(components=[[0,1,2]]),dict(components=[[0,1]]*65),
                dict(components=None),dict(pick='other'),dict(finish='Delete'),dict(view='oblique'),dict(extra=1)]:
            with self.subTest(changes=changes),self.assertRaises(ValueError):validate(dict(base,**changes))
        for changes in [dict(protocol_version=True),dict(iterations=2),dict(operations=[]),dict(operations=[base,base])]:
            client=Mock()
            with self.subTest(changes=changes),self.assertRaises(ValueError):capture(dict(request(),**changes),client)
            client.run_viboceros.assert_not_called();client.run_rhino.assert_not_called()
