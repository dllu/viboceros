"""Block graph validation must precede document and GUI access."""
import copy
import unittest
from unittest.mock import patch

from . import block_workflow_probe as probe
from .client import OracleClient


class BlockWorkflowTests(unittest.TestCase):
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
