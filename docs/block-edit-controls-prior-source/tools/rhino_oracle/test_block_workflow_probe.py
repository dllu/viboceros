"""Block graph validation must precede document and GUI access."""
import copy
import unittest
from unittest.mock import patch

from . import block_workflow_probe as probe
from .client import OracleClient


class BlockWorkflowTests(unittest.TestCase):
    def test_edit_roundtrip_inputs_reject_before_native_host_access(self):
        for step in [dict(action='edit_roundtrip',object=0,translation=[1,2,3],save=True),dict(action='edit_roundtrip',object=True,translation=[1,2,3],save=True),dict(action='edit_roundtrip',object=1,translation=[1,2],save=True),dict(action='edit_roundtrip',object=1,translation=[1,2,3],save=1)]:
            f=self.fixture();f['steps'].append(step)
            with self.subTest(step=step),self.assertRaises(ValueError):probe.run(f,None,{})
    def test_edit_roundtrip_keeps_model_handle_lifetimes(self):
        f=self.fixture();f['steps'].extend([dict(action='edit_roundtrip',object=1,translation=[1,2,3],save=False),dict(action='group',objects=[1])]);probe.validate(f)
    def test_scale_reset_modes_and_handles_reject_before_host_access(self):
        for step in [dict(action='reset_scale',objects=[],mode='one'),dict(action='reset_scale',objects=[0],mode='one'),dict(action='reset_scale',objects=[1,1],mode='one'),dict(action='reset_scale',objects=[1],mode='invalid'),dict(action='reset_scale',objects=[1],mode='one',preselected=1),dict(action='reset_scale',objects=[1],mode='one',preselected=True,cancel=True)]:
            f=self.fixture();f['steps'].append(step)
            with self.subTest(step=step),self.assertRaises(ValueError):probe.run(f,None,{})

    def test_scale_reset_preserves_handle_lifetimes_and_cancelled_targets(self):
        f=self.fixture();f['steps'].extend([dict(action='reset_scale',objects=[1],mode='automatic',cancel=True),dict(action='reset_scale',objects=[1],mode='one'),dict(action='group',objects=[1])]);probe.validate(f)
        f=self.fixture();f['steps'].extend([dict(action='object_state',objects=[1],mode='locked'),dict(action='reset_scale',objects=[1],mode='one')])
        with self.assertRaises(ValueError):probe.run(f,None,{})
    def test_addition_sources_and_target_are_validated_before_host_access(self):
        for target,objects in [(0,[1]),(True,[1]),(2,[]),(2,[2]),(2,[1,1]),(2,[0]),(2,[True]),(2,[99])]:
            f=self.fixture();f['sources'].append(dict(type='point',point=[7,0,0]))
            f['steps'].append(dict(action='add_objects',target=target,objects=objects))
            with self.subTest(target=target,objects=objects),self.assertRaises(ValueError):probe.run(f,None,{})

    def test_addition_consumes_sources_without_allocating_root_handles(self):
        f=self.fixture();f['sources'].append(dict(type='point',point=[7,0,0]))
        f['steps'].extend([dict(action='add_objects',target=2,objects=[1]),dict(action='group',objects=[2])]);probe.validate(f)
        f['steps'].append(dict(action='group',objects=[1]))
        with self.assertRaises(ValueError):probe.run(f,None,{})

    def test_addition_rejects_protected_sources_and_targets(self):
        for handle in (1,2):
            f=self.fixture();f['sources'].append(dict(type='point',point=[7,0,0]))
            f['steps'].extend([dict(action='object_state',objects=[handle],mode='locked'),dict(action='add_objects',target=2,objects=[1])])
            with self.subTest(handle=handle),self.assertRaises(ValueError):probe.run(f,None,{})

    def test_addition_rejects_indirect_definition_cycles(self):
        f=self.fixture();f['steps'].extend([dict(action='create',name='parent',base=[0,0,0],sources=[1]),
            dict(action='insert',name='leaf',transform=[[1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,0,1]]),
            dict(action='add_objects',target=3,objects=[2])])
        with self.assertRaises(ValueError):probe.run(f,None,{})

    def test_replacement_instance_handle_must_be_live_and_use_the_expected_definition(self):
        for replacement in (True, 0, 2, 99, 3.0):
            f=self.fixture();f['sources'].append(dict(type='point',point=[7,0,0]))
            f['steps'].extend([dict(action='create',name='target',base=[0,0,0],sources=[1]),
                              dict(action='replace_block',objects=[2],name='target',replacement_instance=replacement)])
            with self.subTest(replacement=replacement),self.assertRaises(ValueError):probe.run(f,None,{})

    def test_replacement_instance_can_reference_a_name_unsuitable_for_a_macro_token(self):
        f=self.fixture();f['sources'].append(dict(type='point',point=[7,0,0]))
        f['steps'].extend([dict(action='create',name='Target = 10',base=[0,0,0],sources=[1]),
                          dict(action='replace_block',objects=[2],name='Target = 10',replacement_instance=3)])
        probe.validate(f)

    def test_protected_replacement_sources_and_target_handles_reject_before_host(self):
        for handle in (2,3):
            for mode in ('hidden','locked'):
                f=self.fixture();f['sources'].append(dict(type='point',point=[7,0,0]))
                f['steps'].extend([dict(action='create',name='target',base=[0,0,0],sources=[1]),
                                  dict(action='object_state',objects=[handle],mode=mode),
                                  dict(action='replace_block',objects=[2],name='target',replacement_instance=3)])
                with self.subTest(handle=handle,mode=mode),self.assertRaises(ValueError):probe.run(f,None,{})

    def test_normal_state_restores_target_getter_eligibility(self):
        f=self.fixture();f['sources'].append(dict(type='point',point=[7,0,0]))
        f['steps'].extend([dict(action='create',name='target',base=[0,0,0],sources=[1]),
                          dict(action='object_state',objects=[3],mode='hidden'),
                          dict(action='object_state',objects=[3],mode='locked'),
                          dict(action='object_state',objects=[3],mode='normal'),
                          dict(action='replace_block',objects=[2],name='target',replacement_instance=3)])
        probe.validate(f)

    def test_replace_and_state_inputs_are_validated_before_host_access(self):
        for step in(dict(action='replace_block',objects=[],name='leaf'),dict(action='replace_block',objects=[0],name='leaf'),dict(action='replace_block',objects=[1,1],name='leaf'),dict(action='replace_block',objects=[1],name='missing'),dict(action='replace_block',objects=[1],name='leaf',all_instances=1),dict(action='object_state',objects=[1],mode='secret'),dict(action='object_state',objects=[True],mode='hidden')):
            f=self.fixture();f['steps'].append(step)
            with self.subTest(step=step),self.assertRaises(ValueError):probe.run(f,None,{})
    def test_replace_all_updates_symbolic_peers_without_allocating_handles(self):
        f=self.fixture();f['sources'].append(dict(type='point',point=[7,0,0]));f['steps'].extend([dict(action='create',name='target',base=[0,0,0],sources=[1]),dict(action='replace_block',objects=[2],name='target',all_instances=True),dict(action='delete_definition',name='leaf'),dict(action='group',objects=[2,3])]);probe.validate(f)
    def test_unique_selection_and_duplicate_names_are_validated_before_host(self):
        for step in (dict(action='make_unique',objects=[],name='New'),dict(action='make_unique',objects=[0],name='New'),
                     dict(action='make_unique',objects=[1,1],name='New'),dict(action='make_unique',objects=[1],name='bad name'),
                     dict(action='make_unique',objects=[1],name='leaf'),dict(action='duplicate_definition',name='leaf',new_name='leaf'),
                     dict(action='duplicate_definition',name='missing',new_name='Copy')):
            f=self.fixture();f['steps'].append(step)
            with self.subTest(step=step),self.assertRaises(ValueError):probe.run(f,None,{})

    def test_unique_definition_does_not_allocate_new_logical_object_handles(self):
        f=self.fixture();f['steps'].extend([dict(action='make_unique',objects=[1],name='Unique'),dict(action='group',objects=[1]),dict(action='delete_definition',name='leaf')]);probe.validate(f)
    def test_management_names_and_deletion_expectations_precede_host_access(self):
        for step in (dict(action='rename_definition',name='missing',new_name='new'),
                     dict(action='rename_definition',name='leaf',new_name=''),
                     dict(action='delete_definition',name='missing'),
                     dict(action='delete_definition',name='leaf',expect_failure=True),
                     dict(action='delete_definition',name='leaf',expect_failure=1)):
            f=self.fixture();f['steps'].append(step)
            with self.subTest(step=step),self.assertRaises(ValueError):probe.run(f,None,{})
        f=self.fixture();f['record_management']=1
        with self.assertRaises(ValueError):probe.run(f,None,{})

    def test_renaming_updates_nested_symbolic_references_and_deleted_handles_stay_dead(self):
        f=self.fixture();f['steps'].extend([
            dict(action='rename_definition',name='leaf',new_name='New leaf'),
            dict(action='create',name='parent',base=[0,0,0],sources=[1]),
            dict(action='delete_definition',name='New leaf',expect_failure=True),
            dict(action='delete_definition',name='parent'),
            dict(action='delete_definition',name='New leaf')])
        probe.validate(f)
        f['steps'].append(dict(action='group',objects=[2]))
        with self.assertRaises(ValueError):probe.run(f,None,{})

    def test_hidden_source_id_marker_is_normalized_only_for_owned_objects(self):
        key='$block-instance-original-object-id$'
        pairs={key:'01234567-89AB-CDEF-0123-456789abcdef','$custom$':'keep','Part':'member'}
        cleaned=probe._user_text_record(pairs,['01234567-89ab-cdef-0123-456789abcdef'])
        self.assertEqual(cleaned,{'$custom$':'keep','Part':'member'})
        self.assertIn(key,pairs)
        self.assertEqual(probe._user_text_record(pairs,['unrelated']),pairs)

    def test_group_members_and_output_options_are_typed_before_host_access(self):
        for bad in (dict(action='group',objects=[]),dict(action='group',objects=[True]),
                    dict(action='group',objects=[0,0]),dict(action='group',objects=[100]),
                    dict(action='explode',object=1,recursive=False,group_output=True),
                    dict(action='explode',object=1,recursive=True,api='sdk',group_output=True),
                    dict(action='explode',object=1,recursive=True,group_output=1)):
            f=self.fixture()
            f['steps'].append(bad)
            with self.subTest(bad=bad),self.assertRaises(ValueError): probe.run(f,None,{})
        f=self.fixture()
        f['record_groups']=1
        with self.assertRaises(ValueError): probe.run(f,None,{})

    def test_group_step_does_not_allocate_a_new_object_handle(self):
        f=self.fixture()
        f['steps'].insert(0,dict(action='group',objects=[0]))
        f['steps'].append(dict(action='explode',object=1,recursive=True,group_output=True))
        probe.validate(f)

    def test_command_creation_and_batch_selection_are_validated_before_host(self):
        for update in ({'api':'command','name':'part name'}, {'api':'unknown'}, {'api':'command','name':'name\nDelete'}):
            f=self.fixture();f['steps'][0].update(update)
            with self.subTest(update=update),self.assertRaises(ValueError): probe.run(f,None,{})
        for objects in ([True],[0],[1,1],[],[100]):
            f=self.fixture();f['steps'].append(dict(action='explode_batch',objects=objects))
            with self.subTest(objects=objects),self.assertRaises(ValueError): probe.run(f,None,{})

    def fixture(self):
        return dict(op='block_workflow', id='case', sources=[dict(type='point', point=[1, 2, 3])],
                    steps=[dict(action='create', name='leaf', base=[0, 0, 0], sources=[0])])

    def test_valid_nested_handles_redefinition_and_reflection(self):
        f = self.fixture()
        f['sources'].append(dict(type='point', point=[4, 5, 6]))
        f['steps'].extend([
            dict(action='insert', name='LEAF', transform=[[-1, 0, 0, 4], [0, 2, 0, 5], [0, 0, 3, 6], [0, 0, 0, 1]]),
            dict(action='create', name='wrapper', base=[0, 0, 0], sources=[3]),
            dict(action='create', name='leaf', base=[0, 0, 0], sources=[1]),
            dict(action='explode', object=4, recursive=True)])
        probe.validate(f)

    def test_invalid_steps_cannot_touch_host(self):
        for second in (
            dict(action='create', name='other', base=[0, 0, 0], sources=[0]),
            dict(action='create', name='leaf', base=[0, 0, 0], sources=[1]),
            dict(action='create', name='empty', base=[0, 0, 0], sources=[]),
            dict(action='insert', name='missing', transform=[[1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,0,1]]),
            dict(action='insert', name='leaf', transform=[[0,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,0,1]]),
            dict(action='insert', name='leaf', transform=[[1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,1,1]]),
            dict(action='explode', object=True), dict(action='explode', object=1, recursive=1),
            dict(action='explode', object=1, api='arbitrary_command'),
            dict(action='create', name='bad\0name', base=[0,0,0], sources=[1]), dict(action='delete', object=1)
        ):
            f = self.fixture()
            f['steps'].append(second)
            with self.subTest(second=second), self.assertRaises(ValueError):
                probe.run(f, None, {})

    def test_nan_and_bad_metadata_are_rejected_before_host(self):
        for update in ({'attributes':[{'color':[1,2,True]}]}, {'attributes':[{'geometry_user_text':{'a':None}}]},
                       {'attributes':[{'color_source':'material'}]}, {'attributes':[{},{}]}):
            f = dict(self.fixture(), **update)
            with self.subTest(update=update), self.assertRaises(ValueError): probe.run(f,None,{})
        f = self.fixture()
        f['steps'][0]['base'][0] = float('nan')
        with self.assertRaises(ValueError): probe.run(f,None,{})

    def test_expansion_is_bounded_before_host(self):
        f = self.fixture()
        f['steps'].extend([
            dict(action='insert', name='leaf', transform=[[1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,0,1]]),
            dict(action='create', name='double', base=[0,0,0], sources=[1,2])])
        with patch.object(probe, 'MAX_HANDLES', 2), self.assertRaisesRegex(ValueError, 'budget'):
            probe.run(f, None, {})
        f = self.fixture()
        with patch.object(probe, 'MAX_RECORDS', 1), self.assertRaisesRegex(ValueError, 'recording budget'):
            probe.run(f, None, {})

    def test_host_driver_validates_before_starting_process(self):
        f = self.fixture()
        f['steps'][0]['sources'] = [True]
        with patch.dict('os.environ', {'DISPLAY':':999', 'VIBOCEROS_ORACLE_HEADLESS':':999'}), \
             patch('tools.rhino_oracle.client.subprocess.Popen') as spawn, self.assertRaises(ValueError):
            OracleClient().run_rhino(dict(protocol_version=1, iterations=1, operations=[f]))
        spawn.assert_not_called()
