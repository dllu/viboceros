"""SSPI diagnostics reject incomplete evidence and preserve the display boundary."""
import copy
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
from unittest import TestCase
from unittest.mock import patch

from .client import OracleError
from .wine_sspi import LIBRARIES, SYMBOLS, PREFIX, parse_response, run, validate_executable


def payload():
    return dict(libraries=[dict(name=name, loaded=True, error=0,
        symbols=[dict(name=s, attempted=True, resolved=True, error=0) for s in SYMBOLS])
        for name in LIBRARIES])


def output(value):
    return 'Wine diagnostic noise\n' + PREFIX + json.dumps(value) + '\r\n'


def executable(machine):
    data = bytearray(70)
    data[:2] = b'MZ'
    struct.pack_into('<I', data, 60, 64)
    data[64:68] = b'PE\0\0'
    struct.pack_into('<H', data, 68, machine)
    return bytes(data)


class WineSspiTests(TestCase):
    def test_retained_native_and_x64_diagnostics_preserve_the_observed_difference(self):
        from tools.verify_provenance import verify
        root = Path(__file__).resolve().parents[2]
        for architecture, passed in [('x86_64', False), ('aarch64', True)]:
            p = root / ('tools/rhino_oracle/observations/wine_sspi_' + architecture + '_20261014.json')
            record = json.loads(p.read_text())
            self.assertEqual(record['architecture'], architecture)
            self.assertEqual(record['passed'], passed)
            result = parse_response(output(dict(libraries=record['libraries'])), 0 if passed else 1)
            self.assertEqual(result['passed'], passed)
            self.assertEqual(verify(record), 1)
        q = json.loads((root / 'docs/oracle-sspi-qualification.json').read_text())
        self.assertFalse(q['rhino_worker_started'])
        self.assertEqual(verify(q), 5)

    def test_success_and_forwarded_initialization_failure_are_distinct(self):
        self.assertTrue(parse_response(output(payload()), 0)['passed'])
        p = payload()
        for s in p['libraries'][0]['symbols']:
            s.update(resolved=False, error=127)
        p['libraries'][1].update(loaded=False, error=998)
        for s in p['libraries'][1]['symbols']:
            s.update(attempted=False, resolved=False)
        result = parse_response(output(p), 1)
        self.assertFalse(result['passed'])
        self.assertEqual(result['libraries'][1]['error'], 998)
        self.assertFalse(result['libraries'][1]['symbols'][0]['attempted'])

    def test_missing_duplicate_malformed_and_inconsistent_records_are_rejected(self):
        for text, code in [('noise only', 1), (output(payload()) * 2, 0),
                           (PREFIX + '{broken', 1), (output(payload()), 1)]:
            with self.subTest(text=text), self.assertRaises(OracleError):
                parse_response(text, code)
        mutations = [
            lambda p: p['libraries'].pop(),
            lambda p: p['libraries'][0].update(name='arbitrary.dll'),
            lambda p: p['libraries'][0].update(loaded=1),
            lambda p: p['libraries'][0].update(error=-1),
            lambda p: p['libraries'][0].update(error=2**32),
            lambda p: p['libraries'][0].update(error=True),
            lambda p: p['libraries'][0].update(error=998),
            lambda p: p['libraries'][0]['symbols'][0].update(attempted=False),
            lambda p: p['libraries'][0]['symbols'][0].update(error=127),
        ]
        for mutate in mutations:
            p = copy.deepcopy(payload()); mutate(p)
            with self.subTest(payload=p), self.assertRaises(OracleError):
                parse_response(output(p), 0)

    def test_compiled_machine_type_is_verified_before_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            p = Path(temporary) / 'probe.exe'
            for machine, arch in ((0x8664, 'x86_64'), (0xaa64, 'aarch64')):
                p.write_bytes(executable(machine)); validate_executable(p, arch)
                with self.assertRaises(OracleError):
                    validate_executable(p, 'aarch64' if arch == 'x86_64' else 'x86_64')
            for data in (b'', executable(0x8664)[:68], b'x' * 70):
                p.write_bytes(data)
                with self.assertRaises(OracleError):validate_executable(p, 'x86_64')

    def test_display_and_parameter_validation_precede_all_processes(self):
        if not sys.platform.startswith('linux'):self.skipTest('private Xvfb boundary is Linux-specific')
        with patch.dict(os.environ, {'DISPLAY': ':0', 'VIBOCEROS_ORACLE_HEADLESS': ':101'}), \
                patch('tools.rhino_oracle.wine_sspi.subprocess.run') as start:
            with self.assertRaisesRegex(OracleError, 'dedicated Xvfb'):run()
            for kw in (dict(timeout=float('nan')), dict(timeout=0), dict(architecture='i386')):
                with self.assertRaises(OracleError):run(**kw)
            start.assert_not_called()

    def test_compiler_and_launcher_paths_with_spaces_are_separate_arguments(self):
        launched = []
        def compile(args, **kw):
            launched.append(args)
            Path(args[-1]).write_bytes(executable(0x8664))
            return subprocess.CompletedProcess(args, 0, stdout='', stderr='')
        with patch.dict(os.environ, {'DISPLAY': ':101', 'VIBOCEROS_ORACLE_HEADLESS': ':101'}), \
                patch('tools.rhino_oracle.wine_sspi.subprocess.run', side_effect=compile), \
                patch('tools.rhino_oracle.wine_sspi._run_logged') as launch:
            launch.return_value = subprocess.CompletedProcess([], 0, stdout=output(payload()))
            r = run(compiler=Path('/tool chain/clang'), launcher=Path('/Wine apps/launch.sh'))
        self.assertTrue(r['passed'])
        self.assertEqual(launched[0][0], '/tool chain/clang')
        args = launch.call_args.args[0]
        self.assertEqual(args[:2], ['/Wine apps/launch.sh', 'wine'])
        self.assertTrue(args[2].startswith('Z:\\'))
        self.assertFalse(Path(launched[0][-1]).exists())
