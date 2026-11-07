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

    def test_shift_is_scoped_to_the_owned_click_and_released_after_failure(self):
        import subprocess
        marker='@catenary-click:paired-0'
        request={'operations':[{'id':'paired','actions':[{'click':[1,2,0],'shift':True}]}]}
        for fail in [False,True]:
            with self.subTest(fail=fail),tempfile.TemporaryDirectory()as directory:
                picker=CatenaryPointPicker(request);picker.job=Path(directory)
                (picker.job/'catenary-click-ready-paired-0.json').write_text(json.dumps(marker))
                def send(command,**kwargs):
                    if fail and command==['xdotool','click','1']:raise subprocess.CalledProcessError(1,command)
                with mock.patch('tools.rhino_oracle.catenary_point_picker.subprocess.run',side_effect=send)as run:
                    if fail:
                        with self.assertRaises(subprocess.CalledProcessError):picker.send_input(marker,'1','2','owned')
                    else:self.assertTrue(picker.send_input(marker,'1','2','owned'))
                    self.assertEqual([c.args[0]for c in run.call_args_list],[
                        ['xdotool','windowactivate','--sync','owned','keydown','Shift_L'],
                        ['xdotool','click','1'],['xdotool','keyup','Shift_L'],
                    ])

    def test_alt_moves_inside_the_owned_click_scope_and_releases_all_modifiers(self):
        import subprocess
        marker='@catenary-click:angle-0'
        request={'operations':[{'id':'angle','actions':[{'click':[1,2,0],'shift':True,'alt':True}]}]}
        for fail in [False,True]:
            with self.subTest(fail=fail),tempfile.TemporaryDirectory()as directory:
                picker=CatenaryPointPicker(request);picker.job=Path(directory)
                (picker.job/'catenary-click-ready-angle-0.json').write_text(json.dumps(marker))
                def send(command,**kwargs):
                    if fail and 'click'in command:raise subprocess.CalledProcessError(1,command)
                with mock.patch('tools.rhino_oracle.catenary_point_picker.subprocess.run',side_effect=send)as run:
                    if fail:
                        with self.assertRaises(subprocess.CalledProcessError):picker.send_input(marker,'1','2','owned')
                    else:self.assertTrue(picker.send_input(marker,'1','2','owned'))
                    self.assertEqual([c.args[0]for c in run.call_args_list],[
                        ['xdotool','windowactivate','--sync','owned','keydown','Shift_L','Alt_L'],
                        ['xdotool','mousemove','2','2','mousemove','1','2','sleep','0.1','click','1'],
                        ['xdotool','keyup','Alt_L','Shift_L'],
                    ])
