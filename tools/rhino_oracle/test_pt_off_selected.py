"""Selected owner display, exact native producers and command-start history effects."""
import copy
import gzip
import hashlib
import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock
from .client import OracleClient, OracleProtocolError, _compare_value
from .generate_pt_off_selected import request as initial
from .generate_pt_off_selected_qualified import request as qualified
from .generate_pt_off_selected_history import request as history
from .generate_pt_off_selected_lifecycle import request as lifecycle
from .generate_pt_off_selected_undo import request as undo
from .generate_pt_off_selected_undo_qualified import request as undo_qualified
from .generate_pt_off_selected_command_history import request as command_history
from .pt_off_selected_probe import validate
from .structural_sdk import stage_helpers, validate_operations

HERE = Path(__file__).resolve().parent
FAMILIES = [('pt_off_selected', initial,27), ('pt_off_selected_qualified',qualified,31),
            ('pt_off_selected_history',history,9), ('pt_off_selected_lifecycle',lifecycle,10),
            ('pt_off_selected_undo',undo,9),('pt_off_selected_undo_qualified',undo_qualified,9),
            ('pt_off_selected_command_history',command_history,9)]
def rows(family):
    return {r['id']:r['value'] for r in json.loads((HERE/'observations'/(family+'.json')).read_text())['results']}

class PtOffSelectedTests(unittest.TestCase):
    def test_complete_recipes_and_content_addressed_native_producers(self):
        for family,make,count in FAMILIES:
            path=HERE/'fixtures'/(family+'.json');q=json.loads(path.read_text())
            self.assertEqual(q,make());validate_operations(q)
            r=json.loads((HERE/'observations'/(family+'.json')).read_text())
            self.assertEqual(len(r['results']),count)
            self.assertEqual([o['id'] for o in q['operations']],[o['id']for o in r['results']])
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
            for name,digest in r['provenance']['sources'].items():
                data=(HERE/name).read_bytes()
                if hashlib.sha256(data).hexdigest()!=digest:
                    data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),digest,name)
    def test_getters_accept_current_owners_and_keep_unrelated_grips_and_peers(self):
        data=rows('pt_off_selected_qualified')
        for kind in ['line','curve','rational','surface','mesh','subd']:
            modes=['control'] if kind in ['surface','mesh']else['control','edit']
            for mode in modes:
                owner=data[kind+'-'+mode+'-owner'];grip=data[kind+'-'+mode+'-grip']
                self.assertEqual(owner['command_result'],'Success');self.assertFalse(owner['getter_opened'])
                self.assertFalse(owner['after'][0]['on']);self.assertTrue(owner['after'][0]['selected'])
                self.assertEqual(owner['before'][1],owner['after'][1])
                self.assertEqual(grip['command_result'],'Cancel');self.assertTrue(grip['getter_opened'])
                self.assertEqual(grip['before'],grip['after'])
        mixed=data['curve-control-mixed'];self.assertEqual(mixed['before'][1],mixed['after'][1])
        self.assertTrue(mixed['after'][1]['grips'][0]['selected'])
        self.assertTrue(data['curve-control-owner-peer']['peer_selected'])
        self.assertEqual(data['curve-control-off-owner']['before'],data['curve-control-off-owner']['after'])
        self.assertTrue(data['curve-control-off-owner']['getter_opened'])
        self.assertTrue(data['curve-control-group-owner']['after'][1]['on'])
        self.assertTrue(data['curve-control-owner-hidden-peer']['after'][1]['grips'][1]['hidden'])
        for family in ['pt_off_selected_qualified','pt_off_selected_lifecycle']:
            for value in rows(family).values():
                self.assertTrue(value['unchanged']);self.assertTrue(value['redo_witness_confirmed']);self.assertFalse(value['redo_preserved'])
    def test_postselection_acceptance_and_cancel_preserve_unrelated_selection(self):
        for name,v in rows('pt_off_selected_lifecycle').items():
            self.assertTrue(v['getter_opened']);self.assertFalse(v['after'][0]['selected'])
            self.assertEqual(v['before'][1],v['after'][1])
            accepted='postselect-' in name
            self.assertEqual(v['command_result'],'Success'if accepted else'Cancel')
            self.assertEqual(v['after'][0]['on'],not accepted)
            if not accepted:self.assertEqual(v['before'],v['after'])
            self.assertEqual(v['peer_selected'],name.endswith('-peer'))
    def test_history_controls_distinguish_selection_from_command_start(self):
        for family in ['pt_off_selected_history','pt_off_selected_undo','pt_off_selected_undo_qualified','pt_off_selected_command_history']:
            for name,v in rows(family).items():
                command=name.endswith('-PtOffSelected')
                self.assertEqual(v['redo_preserved'],not command)
                self.assertTrue(v['redo_witness_confirmed']);self.assertFalse(v['api_redo'])
                self.assertEqual('Nothing to redo.' in v['history'],command)
                if family!='pt_off_selected_history':
                    self.assertEqual(v['undo_called'],command)
                    self.assertFalse(v['undo_witness_removed'])
                    self.assertTrue(v['undo_sources_unchanged'])
                if family in ['pt_off_selected_undo_qualified','pt_off_selected_command_history']:
                    self.assertTrue(v['undo_setup_removed']);self.assertTrue(v['undo_setup_restored'])
                    self.assertFalse(v['second_undo_removed'])
                    self.assertEqual(v['after'],v['undo_state']);self.assertEqual(v['after'],v['second_undo_state'])
                    self.assertEqual('Undoing PtOffSelected' in v['undo_history'],command)
                    self.assertIn('Nothing to undo.',v['undo_history'])
    def test_initial_capture_is_retained_as_a_diagnostic_not_a_selection_reference(self):
        first=rows('pt_off_selected');qualified_rows=rows('pt_off_selected_qualified')
        self.assertFalse(first['curve-control-owner']['after'][0]['selected'])
        self.assertTrue(qualified_rows['curve-control-owner']['after'][0]['selected'])
        self.assertFalse(first['curve-control-mixed']['after'][1]['grips'][0]['selected'])
        self.assertTrue(qualified_rows['curve-control-mixed']['after'][1]['grips'][0]['selected'])
    def test_closed_recipes_and_standalone_helper_staging(self):
        q=qualified()
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(q,HERE,Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()}, {'pt_off_selected_probe.py','hide_pt_input.py','join_probe.py','merge_edges_probe.py'})
        for value in [None,[],True,3,'PtOffSelected']:
            with self.assertRaises(ValueError):validate(value)
        for patch in [dict(id='bad _Delete'),dict(kind='curve _Delete'),dict(mode='surface'),dict(pattern=True),dict(pattern=[]),dict(extra=True),dict(kind='mesh',mode='edit')]:
            with self.subTest(patch=patch),self.assertRaises(ValueError):validate(dict(q['operations'][0],**patch))
    def test_private_single_family_single_iteration_preflight_runs_before_launch(self):
        q=qualified()
        cases=[(None,q),('VibocerosOraclePtTest',dict(q,iterations=2)),('VibocerosOraclePtTest',dict(q,iterations=True)),
               ('VibocerosOraclePtTest',dict(q,operations=q['operations']*3)),
               ('VibocerosOraclePtTest',dict(q,operations=q['operations']+[history()['operations'][0]]))]
        for scheme,query in cases:
            with (mock.patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),
                  mock.patch('tools.rhino_oracle.client._rhino_process_ids',return_value=set()),
                  mock.patch('tools.rhino_oracle.client._run_logged')as launch,
                  self.assertRaisesRegex(OracleProtocolError,'private dedicated')):
                OracleClient(launcher='/bin/true',settings_scheme=scheme).run_rhino(query)
            launch.assert_not_called()
    def test_complete_comparison_detects_every_point_and_state_field(self):
        original=rows('pt_off_selected_qualified')['curve-control-grip']
        changes=[('point',lambda v:v['after'][0]['grips'][0]['point'].__setitem__(2,0.01)),
                 ('index',lambda v:v['after'][0]['grips'][0].__setitem__('index',1)),
                 ('selected',lambda v:v['after'][0]['grips'][0].__setitem__('selected',False)),
                 ('hidden',lambda v:v['after'][0]['grips'][0].__setitem__('hidden',True)),
                 ('on',lambda v:v['after'][0].__setitem__('on',False)),
                 ('owner',lambda v:v['after'][0].__setitem__('selected',True)),
                 ('peer',lambda v:v.__setitem__('peer_selected',True)),
                 ('redo',lambda v:v.__setitem__('redo_preserved',True)),
                 ('getter',lambda v:v.__setitem__('getter_opened',False)),
                 ('result',lambda v:v.__setitem__('command_result','Success'))]
        for name,change in changes:
            value=copy.deepcopy(original);change(value)
            errors=[];_compare_value('value',original,value,1e-7,1e-12,errors)
            self.assertTrue(errors,name)
