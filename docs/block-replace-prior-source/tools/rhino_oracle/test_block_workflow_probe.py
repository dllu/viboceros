"""Block graph validation must precede document and GUI access."""
import copy
import unittest
from unittest.mock import patch

from . import block_workflow_probe as probe
from .client import OracleClient


class BlockWorkflowTests(unittest.TestCase):
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
