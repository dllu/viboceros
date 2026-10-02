"""Shared settings captures must stay isolated, bounded, and complete."""
import copy
import json
import os
from pathlib import Path
import unittest
from unittest.mock import Mock, patch

from .client import OracleClient, OracleProtocolError, _owned_artifact_request
from .copy_options_capture import capture, validate_request
from .copy_options_cases import request
from .copy_options_probe import validate


class CopyOptionsTests(unittest.TestCase):
    def test_independent_saved_fixture_and_owned_paths(self):
        saved = json.loads(Path(__file__).with_name('fixtures').joinpath('copy_options.json').read_text())
        self.assertEqual(saved, request())
        self.assertEqual(len(saved['operations'][0]['steps']), 50)
        with _owned_artifact_request(saved) as prepared:
            validate_request(prepared)
            validate(prepared['operations'][0])
            artifact = prepared['operations'][0]['sources'][0]['brep']['artifact_path']
            self.assertTrue(Path(artifact).parent.is_dir())
        self.assertFalse(Path(artifact).parent.exists())
        self.assertNotIn('artifact_path', json.dumps(saved))

    def test_protocol_types_and_workflow_bounds(self):
        for key, values in [('protocol_version', [True, None, 2, '1']),
                            ('iterations', [True, 0, 2, '1']),
                            ('operations', [None, {}, [], [None], [1], request()['operations'] * 2])]:
            for value in values:
                invalid = request()
                invalid[key] = value
                client = Mock(settings_scheme='VibocerosOracleTest')
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    capture(invalid, client)
                client.run_viboceros.assert_not_called()
                client.run_rhino.assert_not_called()
        with _owned_artifact_request(request()) as prepared:
            operation = prepared['operations'][0]
            for key, values in [('steps', [[], operation['steps'] * 3, None]),
                                ('sources', [[], operation['sources'] * 2, [dict(brep={})]]),
                                ('id', ['unsafe path', True]), ('extra', [True])]:
                for value in values:
                    invalid = copy.deepcopy(operation)
                    invalid[key] = value
                    with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                        validate(invalid)
            for key, value in [('command', 'Delete'), ('copy', 1), ('copy', 'Yes'),
                               ('finish', 'Undo'), ('extra', True)]:
                invalid = copy.deepcopy(operation)
                invalid['steps'][0][key] = value
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    validate(invalid)
            invalid = copy.deepcopy(operation)
            invalid['steps'] = [dict(command='RememberCopyOptions', copy=True, finish='Cancel')]
            with self.assertRaises(ValueError):
                validate(invalid)
            invalid['steps'][0]['copy'] = None
            validate(invalid)

    def test_missing_private_scheme_never_exports_or_launches(self):
        client = Mock(settings_scheme=None)
        with self.assertRaisesRegex(ValueError, 'private scheme'):
            capture(request(), client)
        client.run_viboceros.assert_not_called()
        client.run_rhino.assert_not_called()
        client = OracleClient(launcher='/bin/true')
        with (_owned_artifact_request(request()) as prepared,
              patch.dict(os.environ, {'DISPLAY': ':101', 'VIBOCEROS_ORACLE_HEADLESS': ':101'}),
              patch('tools.rhino_oracle.client._run_logged') as launch,
              self.assertRaisesRegex(OracleProtocolError, 'private Rhino settings scheme')):
            client.run_rhino(prepared)
        launch.assert_not_called()

    def test_incomplete_exports_never_launch_rhino(self):
        client = Mock(settings_scheme='VibocerosOracleTest')
        client.run_viboceros.return_value = dict(protocol_version=1, engine='viboceros', iterations=1, results=[])
        with self.assertRaisesRegex(ValueError, 'incomplete Copy workflow export'):
            capture(request(), client)
        client.run_rhino.assert_not_called()

    def test_raw_status_and_order_are_retained_and_incomplete_steps_rejected(self):
        observed = json.loads(Path(__file__).with_name('observations').joinpath('copy_options.json').read_text())
        client = Mock(settings_scheme='VibocerosOracleTest')
        client.run_viboceros.return_value = dict(protocol_version=1, engine='viboceros', iterations=1,
                                                results=[dict(id='source', value={}, elapsed_ns=0)])
        client.run_rhino.return_value = observed
        self.assertEqual(capture(request(), client), observed)
        rotate = observed['results'][0]['value']['records'][6]
        self.assertFalse(rotate['succeeded'])
        self.assertEqual(rotate['events'][0]['result'], 'Nothing')
        self.assertEqual(len(rotate['after']), 2)
        for mutation in ['missing', 'reordered', 'invalid_status']:
            invalid = copy.deepcopy(observed)
            records = invalid['results'][0]['value']['records']
            if mutation == 'missing':
                records.pop()
            elif mutation == 'reordered':
                records[0], records[1] = records[1], records[0]
            else:
                records[0]['succeeded'] = 1
            client.run_rhino.return_value = invalid
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                capture(request(), client)


if __name__ == '__main__':
    unittest.main()
