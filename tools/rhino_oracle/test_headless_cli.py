"""Live CLI modes must use their own X server on Linux."""

import unittest
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from unittest.mock import patch

from .__main__ import headless_wrapper_argv


class HeadlessCliTests(unittest.TestCase):
    @unittest.skipUnless(all(shutil.which(name) for name in ['xvfb-run', 'Xvfb', 'i3', 'wmctrl']),
                         'Xvfb, i3 and wmctrl are required')
    def test_wrapper_isolates_wayland_sockets_and_cleans_its_runtime(self):
        wrapper = Path(__file__).with_name('run_headless.sh')
        script = '''import json, os, stat, subprocess, time
for _ in range(100):
    wm = subprocess.run(['wmctrl', '-m'], capture_output=True, text=True)
    if wm.returncode == 0: break
    time.sleep(.05)
assert wm.returncode == 0, wm.stderr
assert 'Name: i3' in wm.stdout, wm.stdout
print(json.dumps(dict(display=os.environ['DISPLAY'],
    headless=os.environ['VIBOCEROS_ORACLE_HEADLESS'],
    runtime=os.environ['XDG_RUNTIME_DIR'],
    mode=stat.S_IMODE(os.stat(os.environ['XDG_RUNTIME_DIR']).st_mode),
    desktop={key: os.environ.get(key) for key in
             ['WAYLAND_DISPLAY', 'WAYLAND_SOCKET', 'SWAYSOCK', 'I3SOCK']})))'''
        environment = dict(os.environ, WAYLAND_DISPLAY='wayland-desktop', WAYLAND_SOCKET='9',
                           SWAYSOCK='/desktop/sway.sock', I3SOCK='/desktop/i3.sock')
        result = subprocess.run([str(wrapper), 'exec', sys.executable, '-c', script],
                                env=environment, capture_output=True, text=True, timeout=20)
        self.assertEqual(result.returncode, 0, result.stderr)
        observed = json.loads(result.stdout)
        self.assertEqual(observed['display'], observed['headless'])
        self.assertNotEqual(observed['display'], environment.get('DISPLAY'))
        self.assertEqual(observed['desktop'], dict.fromkeys(observed['desktop']))
        self.assertNotEqual(observed['runtime'], environment.get('XDG_RUNTIME_DIR'))
        self.assertEqual(observed['mode'], 0o700)
        self.assertFalse(Path(observed['runtime']).exists())

    def test_native_kernel_flag_reaches_the_viboceros_client(self):
        from .__main__ import main
        with tempfile.TemporaryDirectory() as temporary:
            request = Path(temporary) / "request.json"
            output = Path(temporary) / "response.json"
            request.write_text(json.dumps(dict(protocol_version=1, iterations=1, operations=[])))
            argv = ["oracle", "viboceros", str(request), "--native-smlib", "--output", str(output)]
            response = dict(protocol_version=1, engine="viboceros", results=[])
            with patch("sys.argv", argv), patch("tools.rhino_oracle.__main__.OracleClient") as client:
                client.return_value.run_viboceros.return_value = response
                self.assertEqual(main(), 0)
            self.assertTrue(client.call_args.kwargs["native_smlib"])
            self.assertEqual(json.loads(output.read_text()), response)

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
