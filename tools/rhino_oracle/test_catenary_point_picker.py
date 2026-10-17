"""Owned Catenary clicks require motion receipts and never send Enter."""
import json
from pathlib import Path
import tempfile
from unittest import TestCase, mock

from .catenary_point_picker import CatenaryPointPicker
from .client import OracleProtocolError


class CatenaryPointPickerTests(TestCase):
    def test_owned_motion_then_click_only_once_with_exact_receipt(self):
        request = {'operations': [{'id': 'case', 'actions': [[0, 0, 0], {'click': [10, -2, 0]}]}]}
        marker = '@catenary-click:case-1'
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            (job / 'worker-progress.log').write_text('PICK ' + marker + ' 300 200\n')
            picker = CatenaryPointPicker(request)
            with mock.patch('tools.rhino_oracle.group_picking.time.monotonic', return_value=10), \
                 mock.patch('tools.rhino_oracle.group_picking._rhino_window_for_pids', return_value=None) as window, \
                 mock.patch('tools.rhino_oracle.catenary_point_picker.subprocess.run') as run:
                picker(job, [])
                window.assert_not_called()
                picker.ready[marker] = 9
                picker(job, [123])
                window.assert_called_once_with([123])
                run.assert_not_called()
            with mock.patch('tools.rhino_oracle.group_picking.time.monotonic', return_value=11), \
                 mock.patch('tools.rhino_oracle.group_picking._rhino_window_for_pids', return_value='owned'), \
                 mock.patch('tools.rhino_oracle.catenary_point_picker.subprocess.run') as run:
                picker(job, [123])
                run.assert_called_once_with([
                    'xdotool', 'windowactivate', '--sync', 'owned',
                    'mousemove', '301', '200', 'mousemove', '300', '200',
                ], check=True, timeout=10)
                self.assertEqual(picker.seen, set())
                with self.assertRaises(OracleProtocolError):
                    picker.record_diagnostics({})
                run.reset_mock()
                ready = job / 'catenary-click-ready-case-1.json'
                ready.write_text(json.dumps('foreign'))
                with self.assertRaises(OracleProtocolError):
                    picker(job, [123])
                run.assert_not_called()
                ready.write_text(json.dumps(marker))
                picker(job, [123])
                picker(job, [123])
                run.assert_called_once_with([
                    'xdotool', 'windowactivate', '--sync', 'owned', 'click', '1',
                ], check=True, timeout=10)
                self.assertEqual(picker.seen, {marker})
                self.assertEqual(json.loads((job / 'click-ack.json').read_text()), marker)
                picker.record_diagnostics({})
                with self.assertRaises(OracleProtocolError):
                    picker.send_input('@catenary-click:foreign-1', '300', '200', 'owned')
