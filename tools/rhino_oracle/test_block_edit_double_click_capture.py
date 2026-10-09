"""The dedicated gesture adapter restores the shared client after each run."""
import unittest
from unittest.mock import patch
from . import block_edit_double_click_capture as capture
from . import client, group_picking


class BlockEditDoubleClickCaptureTests(unittest.TestCase):
    def test_scoped_worker_and_input_adapters_are_restored_after_success(self):
        original_copy = client.shutil.copyfile
        original_input = group_picking.IdlePicker.send_input
        response = {'results': []}
        with patch.object(client.OracleClient, 'run_rhino', return_value=response) as run:
            self.assertIs(capture.capture(123, 'VibocerosOracleGestureTest'), response)
            request = run.call_args.args[0]
            self.assertEqual(request['operations'][0]['id'], 'block-double-click')
            self.assertEqual(run.call_args.kwargs['timeout'], 123)
        self.assertIs(client.shutil.copyfile, original_copy)
        self.assertIs(group_picking.IdlePicker.send_input, original_input)

    def test_adapters_are_restored_when_native_capture_fails(self):
        original_copy = client.shutil.copyfile
        original_input = group_picking.IdlePicker.send_input
        with patch.object(client.OracleClient, 'run_rhino', side_effect=client.OracleError('failure')):
            with self.assertRaises(client.OracleError):
                capture.capture()
        self.assertIs(client.shutil.copyfile, original_copy)
        self.assertIs(group_picking.IdlePicker.send_input, original_input)
