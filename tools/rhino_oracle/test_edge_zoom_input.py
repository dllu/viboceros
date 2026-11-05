"""Owned command events and acknowledged external input avoid nested UI timers."""
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch
from .edge_zoom_input import start
from .edge_zoom_picking import EdgeZoomPicker


class Event:
    def __init__(self):self.handlers = []
    def __iadd__(self, callback):self.handlers.append(callback);return self
    def __isub__(self, callback):self.handlers.remove(callback);return self
    def fire(self, command):
        for callback in list(self.handlers):callback(None,SimpleNamespace(CommandEnglishName=command))


class EdgeZoomInputTests(unittest.TestCase):
    def environment(self):
        event = Event()
        rhino = SimpleNamespace(Commands=SimpleNamespace(Command=SimpleNamespace(BeginCommand=event)))
        system = SimpleNamespace(Guid=SimpleNamespace(NewGuid=lambda:SimpleNamespace(ToString=lambda unused:'a'*32)),
                                 Threading=SimpleNamespace(Thread=SimpleNamespace(Sleep=Mock())))
        return event,rhino,system

    def test_only_zoom_begin_requests_inputs_and_requires_host_receipts(self):
        event,rhino,system = self.environment()
        with tempfile.TemporaryDirectory() as folder, patch('tools.rhino_oracle.edge_zoom_input.__file__', str(Path(folder)/'helper.py')):
            finish = start(rhino,system,['_All','_Mark',''])
            event.fire('Box')
            self.assertFalse((Path(folder)/'worker-progress.log').exists())
            event.fire('ZoomNaked')
            event.fire('ZoomNaked')
            lines = (Path(folder)/'worker-progress.log').read_text().splitlines()
            self.assertEqual(len(lines),3)
            names = [line.split()[1] for line in lines]
            self.assertEqual([name.rsplit(':',1)[1] for name in names],['All','Mark','Enter'])
            (Path(folder)/'edge-zoom-ack.json').write_text(json.dumps(names))
            trace = finish()
            self.assertTrue(all(s['acknowledged'] for s in trace['sent']))
            self.assertEqual([s['token'] for s in trace['sent']],['_All','_Mark',''])
            self.assertEqual(event.handlers,[])
            finish.stop()

    def test_missing_receipts_fail_and_detach_the_command_observer(self):
        event,rhino,system = self.environment()
        with tempfile.TemporaryDirectory() as folder, patch('tools.rhino_oracle.edge_zoom_input.__file__', str(Path(folder)/'helper.py')):
            finish = start(rhino,system,['_All','_Mark',''])
            event.fire('ZoomNaked')
            with self.assertRaisesRegex(ValueError,'owned edge Zoom input failed'):finish()
            self.assertEqual(event.handlers,[])
            self.assertEqual(system.Threading.Thread.Sleep.call_count,200)

    def test_foreign_markers_and_arbitrary_commands_are_rejected(self):
        picker = EdgeZoomPicker()
        with patch('tools.rhino_oracle.edge_zoom_picking.subprocess.run') as run:
            for marker in ('@other:All','@edge-zoom:'+'a'*32+':0:Delete'):
                with self.assertRaises(ValueError):picker.send_input(marker,1,1,'123')
            run.assert_not_called()
            self.assertTrue(picker.send_input('@edge-zoom:'+'a'*32+':0:Mark',1,1,'123'))
            self.assertEqual(run.call_args_list[0].args[0][-1],'_Mark')
            self.assertEqual(run.call_args_list[1].args[0][-1],'Return')


if __name__ == '__main__':
    unittest.main()
