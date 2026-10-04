"""Live CLI modes must use their own X server on Linux."""

import unittest
import json
import os
import tempfile
from pathlib import Path
from unittest.mock import patch

from .__main__ import headless_wrapper_argv


class HeadlessCliTests(unittest.TestCase):
    def test_private_scheme_and_output_file_reach_the_isolated_client(self):
        from .__main__ import main
        with tempfile.TemporaryDirectory() as temporary:
            request = Path(temporary)/'request.json'
            output = Path(temporary)/'response.json'
            request.write_text(json.dumps(dict(protocol_version=1,iterations=1,operations=[])))
            argv = ['oracle','rhino',str(request),'--scheme','VibocerosOracleFit','--output',str(output)]
            response = dict(protocol_version=1,engine='rhino',results=[])
            with patch('sys.argv',argv),patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),patch('tools.rhino_oracle.__main__.OracleClient') as client:
                client.return_value.run_rhino.return_value = response
                self.assertEqual(main(),0)
            self.assertEqual(client.call_args.kwargs['settings_scheme'],'VibocerosOracleFit')
            self.assertEqual(json.loads(output.read_text()),response)

    def test_live_modes_route_through_xvfb_wrapper(self):
        for mode in ("rhino", "compare"):
            argv = [mode, "fixture.json", "--timeout", "300"]
            command = headless_wrapper_argv(mode, argv, {"DISPLAY": ":0"}, "linux")
            self.assertIsNotNone(command)
            self.assertTrue(command[0].endswith("/run_headless.sh"))
            self.assertEqual(command[1:], argv)

    def test_replay_and_isolated_runs_do_not_reexec(self):
        self.assertIsNone(headless_wrapper_argv("replay", ["replay", "fixture.json"], {}, "linux"))
        self.assertIsNone(headless_wrapper_argv("rhino", ["rhino", "fixture.json"],
                                                 {"DISPLAY": ":101", "VIBOCEROS_ORACLE_HEADLESS": ":101"}, "linux"))
        self.assertIsNotNone(headless_wrapper_argv("rhino", ["rhino", "fixture.json"],
                                                    {"DISPLAY": ":0", "VIBOCEROS_ORACLE_HEADLESS": ":101"}, "linux"))
        self.assertIsNotNone(headless_wrapper_argv("rhino", ["rhino", "fixture.json"],
                                                    {"VIBOCEROS_RHINO_VISIBLE": "1"}, "linux"))
        self.assertIsNone(headless_wrapper_argv("rhino", ["rhino", "fixture.json"], {}, "darwin"))


if __name__ == "__main__":
    unittest.main()
