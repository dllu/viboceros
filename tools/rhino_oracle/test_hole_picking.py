import tempfile
from pathlib import Path
import unittest
from unittest.mock import patch

from .hole_picking import HolePicker


class HolePickingTests(unittest.TestCase):
    def test_drag_scopes_activation_and_releases_button_and_modifiers_on_failure(self):
        for fail in (False, True):
            with patch('tools.rhino_oracle.hole_picking.subprocess.run') as run, patch('tools.rhino_oracle.hole_picking.time.sleep'):
                if fail: run.side_effect = [None, RuntimeError('injected motion failure'), None, None]
                picker = HolePicker()
                if fail:
                    with self.assertRaises(RuntimeError): picker.send_input('@hole-window:owned:sub:30:40','10','20','123')
                else: self.assertTrue(picker.send_input('@hole-window:owned:sub:30:40','10','20','123'))
                commands = [call.args[0] for call in run.call_args_list]
                self.assertEqual(commands[1][:4], ['xdotool','windowactivate','--sync','123'])
                self.assertEqual(commands[-2], ['xdotool','mouseup','1'])
                self.assertEqual(commands[-1], ['xdotool','keyup','shift','keyup','ctrl'])
                for call in run.call_args_list: self.assertEqual(call.kwargs, dict(check=True, timeout=10))

    def test_progress_cannot_drive_foreign_windows(self):
        with tempfile.TemporaryDirectory() as tmp, patch('tools.rhino_oracle.group_picking.time.monotonic',return_value=10), patch('tools.rhino_oracle.group_picking._rhino_window_for_pids',return_value=None) as window, patch('tools.rhino_oracle.hole_picking.subprocess.run') as run:
            job = Path(tmp); (job/'worker-progress.log').write_text('PICK @hole-window:owned:plain:30:40 10 20\n')
            picker = HolePicker(); picker.ready['@hole-window:owned:plain:30:40'] = 0
            picker(job,set()); window.assert_not_called(); run.assert_not_called()
            picker(job,{456}); window.assert_called_once_with({456}); run.assert_not_called()

    def test_finish_and_markers_validate_without_sending_malformed_input(self):
        with patch('tools.rhino_oracle.hole_picking.subprocess.run') as run:
            picker = HolePicker()
            for finish,key in [('Enter','Return'),('Cancel','Escape')]:
                picker.send_input('@hole-finish:owned:'+finish,'1','1','123')
                self.assertEqual(run.call_args.args[0],['xdotool','windowactivate','--sync','123','key','--clearmodifiers',key])
            run.reset_mock()
            for name in ['@hole-window:owned:other:1:2','@hole-window:owned:sub:-1:2','@hole-finish:owned:Undo']:
                with self.assertRaises(ValueError): picker.send_input(name,'1','2','123')
            run.assert_not_called()


if __name__ == '__main__': unittest.main()
