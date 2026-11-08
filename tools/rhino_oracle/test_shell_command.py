"""Private Shell routing and standalone worker compatibility regressions."""
import copy
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from unittest import TestCase, mock
from .client import OracleClient, OracleError
from .shell_command import PILOT, request, validate
from .structural_sdk import stage_helpers, validate_operations


class ShellCommandTests(TestCase):
    def test_fixed_recipes_reject_unknown_and_mutable_operations(self):
        query = request(list(PILOT))
        validate_operations(query)
        for key, value in [('id', 'another'), ('case', 'unknown'), ('extra', True)]:
            operation = copy.deepcopy(query['operations'][0]); operation[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                validate(operation)

    def test_standalone_dispatch_accepts_shell_before_accessing_rhino(self):
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(request(list(PILOT)), Path(__file__).parent, Path(directory))
            script = '''import sys
sys.path.insert(0, sys.argv[1])
import conic_getter_input, shell_command_probe
shell_command_probe.validate(dict(op='shell_command',id='shell_getter_keep',case='shell_getter_keep'))
try:
    conic_getter_input.drive({}, {}, [], 'Shell')
except KeyError as error:
    assert error.args == ('Rhino',)
else:
    raise AssertionError('dispatch did not enter the Shell driver')
'''
            subprocess.run([sys.executable, '-I', '-c', script, directory],
                           check=True, capture_output=True, text=True)

    def test_sdk_probe_imports_without_pathlib_or_math_isfinite(self):
        script = '''import sys, math
del math.isfinite
sys.modules['pathlib'] = None
sys.path.insert(0, sys.argv[1])
import shell_probe
shell_probe.validate(dict(op='shell',id='cap',source=dict(type='box_brep'),
                         opening_points=[[5,4,6]],thickness=1.0))
'''
        subprocess.run([sys.executable, '-I', '-c', script, str(Path(__file__).parent)],
                       check=True, capture_output=True, text=True)

    def test_actual_client_requires_a_private_scheme_and_stages_shell(self):
        query = request(list(PILOT))
        with mock.patch.dict(os.environ, {'DISPLAY': ':101', 'VIBOCEROS_ORACLE_HEADLESS': ':101'}):
            with self.assertRaisesRegex(OracleError, 'private scheme'):
                OracleClient(launcher='/bin/true').run_rhino(query)
            paths = []

            def inspect(command, cwd, timeout, log_path):
                job = log_path.parent; paths.append(job)
                self.assertEqual((job/'shell_command_probe.py').read_bytes(),
                                 Path(__file__).with_name('shell_command_probe.py').read_bytes())
                self.assertIn('Shell', (job/'conic_getter_input.py').read_text())
                raise OracleError('owned Shell staging verified')

            with (mock.patch('tools.rhino_oracle.client._rhino_process_ids', return_value=set()),
                  mock.patch('tools.rhino_oracle.client._run_logged', side_effect=inspect),
                  self.assertRaisesRegex(OracleError, 'owned Shell staging verified')):
                OracleClient(launcher='/bin/true',
                             settings_scheme='VibocerosOracleShellTest').run_rhino(query)
            self.assertEqual(len(paths), 1)
            self.assertFalse(paths[0].exists())
