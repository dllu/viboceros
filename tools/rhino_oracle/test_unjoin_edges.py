import copy
import json
from pathlib import Path
import unittest
from unittest.mock import Mock

from .client import OracleError, OracleProtocolError
from .unjoin_edges_cases import request as api_request
from .unjoin_edge_command_cases import request as command_request
from .unjoin_edge_selection_cases import request as selection_request
from .unjoin_edges_capture import capture
from .unjoin_edges_probe import validate as validate_api
from .unjoin_edge_command_probe import validate as validate_command
from .unjoin_edges_replay import replay

ROOT = Path(__file__).parent


class UnjoinEdgesTests(unittest.TestCase):
    def test_selection_sequences_regenerate_and_preserve_each_native_step(self):
        fixture=json.loads((ROOT/'fixtures/unjoin_edge_selection.json').read_text())
        observed=json.loads((ROOT/'observations/unjoin_edge_selection.json').read_text())
        self.assertEqual(selection_request(),fixture)
        self.assertEqual(len(observed['results']),42)
        values={row['id']:row['value'] for row in observed['results']}
        for op,row in zip(fixture['operations'],observed['results']):
            self.assertEqual(op['id'],row['id']);value=row['value']
            self.assertEqual(len(value['selection_steps']),len(op['steps']))
            self.assertEqual(value['component_selection']['before'],[])
            self.assertEqual(value['component_selection']['after'],[])
            for selected in value['selection_steps']:
                self.assertEqual(selected,sorted(selected))
                self.assertEqual(len(selected),len(set(map(tuple,selected))))
                for source,kind,index in selected:
                    self.assertEqual(kind,'edge')
                    self.assertLess(index,len(value['constructed'][source]['definition']['edges']))
            if value['history_tested']:
                self.assertTrue(value['succeeded'])
                self.assertEqual(value['undo'],value['before']);self.assertEqual(value['redo'],value['after'])
            else:self.assertEqual(value['before'],value['after'])
        both=[[0,'edge',2],[0,'edge',6]]
        for name in ('click-ctrl','click-sub','window-ctrl','cross-ctrl'):
            self.assertEqual(values[name]['selection_steps'][-1],[[0,'edge',6]])
        for name in ('window-sub','cross-sub','readd','sub-repeat','window-alt'):
            self.assertEqual(values[name]['selection_steps'][-1],both)
        self.assertEqual(values['partial-window']['selection_steps'],[[]])
        self.assertEqual(values['partial-cross']['selection_steps'],[[[0,'edge',2]]])
        self.assertEqual(values['unselected-ctrl']['selection_steps'][-1],[[0,'edge',2]])
        self.assertEqual(values['unselected-alt']['selection_steps'][-1],[[0,'edge',2]])
        self.assertEqual(values['empty-alt']['selection_steps'],[[]])
        self.assertEqual(values['key-Undo']['selection_steps'][1:], [both,both,both])
        for name in ('key-None','key-None-empty'):
            self.assertFalse(values[name]['succeeded']);self.assertFalse(values[name]['history_tested'])
            self.assertEqual(values[name]['selection_steps'][-1],[])

    def test_sequence_validation_rejects_unbounded_or_output_driven_input(self):
        base=selection_request()['operations'][0]
        base['sources'][0]['brep']['artifact_path']='/owned/source.3dm'
        validate_command(base)
        for changes in (dict(steps=[]),dict(steps=None),dict(steps=base['steps']*22),dict(components=[[0,2]]),dict(object_preselect=True),dict(kind='face'),dict(pick='mouse')):
            with self.subTest(changes=changes),self.assertRaises(ValueError):validate_command(dict(base,**changes))
        for step in [dict(kind='click',component=[1,2],modifiers='plain'),dict(kind='click',component=[0,True],modifiers='plain'),dict(kind='click',component=[0,-1],modifiers='plain'),dict(kind='click',component=[0,2],modifiers='invalid'),dict(kind='click',component=[0,2],modifiers='plain',expected=[2]),dict(kind='window',corners=[[0,0,0],[float('nan'),1,0]],modifiers='plain'),dict(kind='window',corners=[[0,0,0],[1e7,1,0]],modifiers='plain'),dict(kind='window',corners=[[0,0,0],[True,1,0]],modifiers='plain'),dict(kind='key',value='_Delete'),dict(kind='key',value='Undo',modifiers='plain'),dict(kind='key',value='None')]:
            with self.subTest(step=step),self.assertRaises(ValueError):validate_command(dict(base,steps=[step]))
        validate_command(dict(base,steps=[dict(kind='key',value='Undo')]))
        validate_command(dict(base,steps=[dict(kind='key',value='None')],finish='Cancel'))
        with self.assertRaises(ValueError):validate_command(dict(base,steps=[dict(kind='key',value='None'),dict(kind='key',value='Undo')],finish='Cancel'))

    def test_intermediate_selection_stays_in_full_saved_comparison(self):
        request=selection_request();observed=json.loads((ROOT/'observations/unjoin_edge_selection.json').read_text())
        client=Mock();client.run_viboceros.return_value=dict(observed,engine='viboceros')
        self.assertTrue(replay(request,observed,client).passed)
        bad=copy.deepcopy(observed);bad['results'][0]['value']['selection_steps'][0]=[]
        self.assertFalse(replay(request,bad,client).passed)

    def test_sources_regenerate_and_native_api_never_mutates_source(self):
        fixture = json.loads((ROOT/'fixtures/brep_unjoin_edges.json').read_text())
        observed = json.loads((ROOT/'observations/brep_unjoin_edges.json').read_text())
        self.assertEqual(api_request(), fixture)
        self.assertEqual(len(observed['results']), 26)
        for op, row in zip(fixture['operations'], observed['results']):
            self.assertEqual(op['id'], row['id'])
            self.assertEqual(row['value']['before'], row['value']['working'])
        values = {row['id']: row['value'] for row in observed['results']}
        for name in ('box-empty','pair-naked','tube-outer-seam','tube-inner-seam','sphere-seam'):
            self.assertEqual(values[name]['after'], [])
        self.assertEqual(len(values['box-all']['after']),6)
        self.assertEqual(len(values['nonmanifold-shared']['after']),3)
        self.assertEqual(len(values['tube-all']['after']),4)

    def test_native_command_finish_selection_metadata_and_history(self):
        fixture = json.loads((ROOT/'fixtures/unjoin_edge_command.json').read_text())
        observed = json.loads((ROOT/'observations/unjoin_edge_command.json').read_text())
        self.assertEqual(command_request(), fixture)
        self.assertEqual(len(observed['results']),48)
        values = {}
        for op, row in zip(fixture['operations'], observed['results']):
            self.assertEqual(op['id'],row['id']); value = row['value']; values[row['id']] = value
            self.assertEqual(value['component_selection']['after'], [])
            if value['history_tested']:
                self.assertTrue(value['succeeded'])
                self.assertEqual(value['undo'],value['before']);self.assertEqual(value['redo'],value['after'])
                self.assertEqual(value['component_selection']['undo'],[]);self.assertEqual(value['component_selection']['redo'],[])
                for obj in value['after']:
                    self.assertFalse(obj['selected'])
                    source = int(obj['name'].split('-')[1])
                    self.assertEqual(obj['groups'],[source]);self.assertEqual(obj['color'],[10+source,30,50])
                    self.assertEqual(obj['color_source'],'ColorFromObject');self.assertTrue(obj['current_layer'])
        self.assertTrue(values['box-cap-Cancel']['succeeded'])
        self.assertEqual(values['box-cap-Enter']['after'],values['box-cap-Cancel']['after'])
        for name in ('box-empty-Enter','pair-naked-Enter','tube-outer-seam-Enter','wrong-face','wrong-object'):
            self.assertFalse(values[name]['succeeded']);self.assertFalse(values[name]['history_tested'])
        for name in ('plane','strip-0','strip-1'):
            self.assertTrue(values['mouse-'+name+'-Enter']['succeeded'])
            cancel=values['mouse-'+name+'-Cancel']
            self.assertFalse(cancel['succeeded']);self.assertEqual(cancel['before'],cancel['after'])

    def test_strict_indices_commands_and_owned_artifacts(self):
        api = dict(op='brep_unjoin_edges',id='owned',source=dict(source=dict(type='box'),artifact_path='/owned/source.3dm'),edges=[0])
        command = dict(op='unjoin_edge_command',id='owned',sources=[dict(brep=api['source'])],components=[[0,0]],finish='Enter',undo_redo=True)
        validate_api(api);validate_command(command)
        for changes in (dict(edges=None),dict(edges=[True]),dict(edges=[-1]),dict(edges=[1.]),dict(edges=[0]*100001),dict(extra=1),dict(id='x _Delete'),dict(op='other'),dict(source={})):
            with self.subTest(changes=changes),self.assertRaises(ValueError):validate_api(dict(api,**changes))
        for changes in (dict(components=None),dict(components=[[1,0]]),dict(components=[[0,True]]),dict(components=[[0,-1]]),dict(components=[[0,0,1]]),dict(components=[[0,0]]*100001),dict(sources=[]),dict(sources=[{}]),dict(finish='_Delete'),dict(kind='vertex'),dict(kind='face',pick='mouse'),dict(pick='window'),dict(undo_redo=1),dict(object_preselect=1),dict(extra=1),dict(id='x _Delete')):
            with self.subTest(changes=changes),self.assertRaises(ValueError):validate_command(dict(command,**changes))

    def test_capture_exports_independent_sources_first_and_cleans_paths(self):
        request=command_request();request['operations'] += api_request()['operations']
        original=copy.deepcopy(request);paths=[];client=Mock(); sequence=[]
        def export(prepared, timeout):
            sequence.append('export')
            for op in prepared['operations']:
                self.assertEqual(op['op'],'brep_remove_holes');self.assertEqual(op['loops'],[])
                path=Path(op['source']['artifact_path']);self.assertTrue(path.parent.is_dir());self.assertFalse(path.exists());paths.append(path)
            return dict(protocol_version=1,engine='viboceros',iterations=1,results=[dict(id=op['id'],elapsed_ns=0,value={}) for op in prepared['operations']])
        def native(prepared,timeout):
            self.assertEqual(sequence,['export']);sequence.append('native');return {'captured':True}
        client.run_viboceros.side_effect=export;client.run_rhino.side_effect=native
        self.assertEqual(capture(request,client),{'captured':True});self.assertEqual(request,original)
        self.assertEqual(len(set(paths)),len(paths));self.assertTrue(all(not path.parent.exists() for path in paths))
        for invalid in (dict(request,iterations=2),dict(request,protocol_version=True),dict(request,operations=[]),dict(request,operations=[request['operations'][0]]*2),dict(request,operations=[dict(op='other')])):
            client.reset_mock()
            with self.assertRaises(ValueError):capture(invalid,client)
            client.run_viboceros.assert_not_called();client.run_rhino.assert_not_called()

    def test_replay_compares_every_geometry_and_component_metadata_field(self):
        for stem,request in [('brep_unjoin_edges',api_request()),('unjoin_edge_command',command_request())]:
            observed=json.loads((ROOT/('observations/'+stem+'.json')).read_text());original=copy.deepcopy(observed)
            client=Mock();client.run_viboceros.return_value=copy.deepcopy(observed)
            client.run_viboceros.return_value['engine']='viboceros'
            self.assertTrue(replay(request,observed,client).passed);self.assertEqual(observed,original)
            for field in ('edge','surface','selection','groups','source'):
                bad=copy.deepcopy(observed);value=next(row['value'] for row in bad['results'] if row['value']['after'])
                if stem=='brep_unjoin_edges':
                    if field not in ('edge','surface'):continue
                    definition=value['after'][0]
                else:
                    obj=value['after'][0];definition=obj['geometry']['definition']
                    if field=='selection':value['component_selection']['after']=[[0,'edge',0]]
                    elif field=='groups':obj['groups']=[]
                    elif field=='source':obj['source']=None
                if field=='edge':definition['edges'][0]['tolerance']+=0.01
                elif field=='surface':definition['faces'][0]['definition']['control_points'][0]['weight']=2.
                self.assertFalse(replay(request,bad,client).passed,field)
            bad=copy.deepcopy(observed);bad['results'][0]['id']='wrong'
            with self.assertRaises(OracleProtocolError):replay(request,bad,client)
            bad=copy.deepcopy(observed);bad['results'].pop()
            with self.assertRaises(OracleProtocolError):replay(request,bad,client)

    def test_failed_or_incomplete_source_exports_never_launch_rhino(self):
        request=api_request();request['operations']=request['operations'][:1]
        for response in (dict(protocol_version=1,engine='viboceros',iterations=1,results=[],error='invalid source'),dict(protocol_version=1,engine='viboceros',iterations=1,results=[])):
            client=Mock();client.run_viboceros.return_value=response
            with self.assertRaises((OracleError,ValueError)):capture(request,client)
            client.run_rhino.assert_not_called()

    def test_large_source_sets_export_in_bounded_batches_before_native_commands(self):
        base=command_request()['operations'][0];base['sources']*=4
        request=dict(protocol_version=1,iterations=1,operations=[dict(copy.deepcopy(base),id='case-%d'%i) for i in range(35)])
        sizes=[];client=Mock()
        def export(prepared,timeout):
            sizes.append(len(prepared['operations']))
            return dict(protocol_version=1,engine='viboceros',iterations=1,results=[dict(id=op['id'],elapsed_ns=0,value={}) for op in prepared['operations']])
        def native(prepared,timeout):self.assertEqual(sizes,[128,12]);return {'captured':True}
        client.run_viboceros.side_effect=export;client.run_rhino.side_effect=native
        self.assertEqual(capture(request,client),{'captured':True});self.assertEqual(sizes,[128,12])


if __name__=='__main__':unittest.main()
