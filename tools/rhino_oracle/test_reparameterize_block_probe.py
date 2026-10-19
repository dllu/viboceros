import copy
import os
from pathlib import Path
import tempfile
from unittest import TestCase, mock
from .client import OracleClient, OracleError
from .reparameterize_block_probe import validate
from .structural_sdk import stage_helpers, validate_operations
from .reparameterize_block_picker import BlockReferencePicker
import json

def request():
    return dict(protocol_version=1,iterations=1,operations=[dict(op='reparameterize_block_command',id='line',family='line',
        placements=[[[1.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,1.,0.],[0.,0.,0.,1.]]],member_path=[0],
        actions=[dict(click=[1.5,2.,0.]),-2.,8.],view='Top',artifact_path='/tmp/owned-block-source.3dm')])

class ReparameterizeBlockProbeTests(TestCase):
    def test_complete_worker_staging_and_bounded_input(self):
        q=request();validate_operations(q)
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(q,Path(__file__).parent,Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()},
                {'reparameterize_block_probe.py','block_workflow_probe.py','conic_getter_input.py',
                 'managed_mouse_callback.py','snap_environment.py','join_probe.py','merge_edges_probe.py'})
        for actions in ([dict(click=[1.5,2.,0.]),'Automatic'],[dict(click=[1.5,2.,0.]),'Cancel'],
                        [dict(click=[1.5,2.,0.]),'Enter','Enter']):
            op=copy.deepcopy(q['operations'][0]);op['actions']=actions;validate(op)
    def test_invalid_paths_matrices_and_inputs_reject_before_host_access(self):
        original=request()['operations'][0]
        for field,value in [('member_path',[True]),('member_path',[2]),('member_path',[]),
                            ('placements',[]),('placements',[[[0.]*4]*4]),
                            ('actions',[dict(click=[0.,float('inf'),0.]),0.,1.]),
                            ('actions',[dict(click=[0.,0.,0.]),1.,0.]),
                            ('actions',[dict(click=[0.,0.,0.]),'Other']),('view','Other')]:
            op=copy.deepcopy(original);op[field]=value
            with self.subTest(field=field,value=value),self.assertRaises(ValueError):validate(op)
        for field,value in [('selection_choices',[0,1]),('selection_choices',[True]),
                            ('alternate_family','Other'),('alternate_domain',[1.,0.])]:
            op=copy.deepcopy(original);op[field]=value
            with self.subTest(field=field),self.assertRaises(ValueError):validate(op)

    def test_multiple_references_share_one_domain_and_mixed_selection_can_end_before_numeric_input(self):
        op=request()['operations'][0]
        op['actions'].insert(1,dict(click=[31.5,2.,0.]))
        validate(op)
        op['alternate_family']='box';op['actions']=op['actions'][:2]
        validate(op)
    def test_private_single_iteration_owned_capture_is_required(self):
        with (mock.patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),
              mock.patch('tools.rhino_oracle.client._rhino_process_ids',return_value=set()),
              mock.patch('tools.rhino_oracle.client._run_logged')as launch):
            with self.assertRaisesRegex(OracleError,'Block reparameterization'):OracleClient().run_rhino(request())
            q=request();q['iterations']=2
            with self.assertRaisesRegex(OracleError,'Block reparameterization'):OracleClient(settings_scheme='VibocerosOracleBlockInput').run_rhino(q)
            launch.assert_not_called()
    def test_selection_confirmation_follows_an_acknowledged_click_and_explicit_choice(self):
        q=request();q['operations'][0]['selection_choice']=1
        picker=BlockReferencePicker(q);marker='@catenary-click:line-0'
        with tempfile.TemporaryDirectory()as directory:
            picker.job=Path(directory)
            with (mock.patch('subprocess.run')as run,mock.patch('time.sleep')):
                self.assertFalse(picker.send_input(marker,'10','20','123'))
                self.assertFalse((picker.job/'block-selection-ready-line-0.json').exists())
                (picker.job/'catenary-click-ready-line-0.json').write_text(json.dumps(marker))
                self.assertTrue(picker.send_input(marker,'10','20','123'))
                self.assertEqual(json.loads((picker.job/'block-selection-ready-line-0.json').read_text()),marker)
                self.assertEqual(run.call_args_list[-1].args[0][-3:],['Home','Down','Return'])
