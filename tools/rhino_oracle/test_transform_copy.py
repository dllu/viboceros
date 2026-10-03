"""Repeated transforms require bounded inputs, private settings, and raw events."""
import copy
import json
import os
from pathlib import Path
import unittest
from unittest.mock import Mock, patch

from .client import OracleClient, OracleProtocolError
from .transform_copy_capture import capture, validate_request
from .transform_copy_cases import request, script_request, center_request, identity_request, default_request
from .transform_copy_probe import validate


class TransformCopyTests(unittest.TestCase):
    def test_script_and_center_workflows_prescribe_inputs_before_measurement(self):
        for name, factory, count in [('transform_copy_script', script_request, 56),
                                     ('transform_copy_center', center_request, 18),
                                     ('transform_copy_identity', identity_request, 64),
                                     ('transform_copy_default', default_request, 80)]:
            saved = json.loads(Path(__file__).with_name('fixtures').joinpath(name+'.json').read_text())
            self.assertEqual(saved, factory())
            self.assertEqual(len(saved['operations']), count)
            validate_request(saved)
            observed = json.loads(Path(__file__).with_name('observations').joinpath(name+'.json').read_text())
            client = Mock(settings_scheme='VibocerosOracleTest', run_rhino=Mock(return_value=observed))
            self.assertEqual(capture(saved, client), observed)

    def test_preselection_and_plane_fields_are_bounded_before_launch(self):
        for key, values in [('selected', [[], [True], [-1], [3], [0, 0], '0']),
                            ('cplane', [None, {}, dict(origin=[0, 0, 0], x_axis=[1, 0, 0], y_axis=[1, 0, 0]),
                                        dict(origin=[0, 0, 0], x_axis=[True, 0, 0], y_axis=[0, 1, 0])])]:
            for value in values:
                invalid = script_request()
                invalid['operations'][0][key] = value
                client = Mock(settings_scheme='VibocerosOracleTest')
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    capture(invalid, client)
                client.run_rhino.assert_not_called()

    def test_fixture_is_independently_prescribed(self):
        saved = json.loads(Path(__file__).with_name('fixtures').joinpath('transform_copy.json').read_text())
        self.assertEqual(saved, request())
        self.assertEqual(len(saved['operations']), 49)
        validate_request(saved)

    def test_protocol_bounds_and_duplicate_ids_fail_before_launch(self):
        for key, values in [('protocol_version', [True, None, 2, '1']),
                            ('iterations', [True, 0, 2, '1']),
                            ('operations', [None, {}, [], [None], [1], request()['operations'] * 3])]:
            for value in values:
                invalid = request()
                invalid[key] = value
                client = Mock(settings_scheme='VibocerosOracleTest')
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    capture(invalid, client)
                client.run_rhino.assert_not_called()
        invalid = request()
        invalid['operations'][1]['id'] = invalid['operations'][0]['id']
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            validate_request(invalid)

    def test_macro_tokens_are_bounded_numbers_and_copy_choices(self):
        operation = request()['operations'][0]
        for key, values in [('command', ['Delete', None]), ('id', ['unsafe path', True]),
                            ('sources', [[], [[True, 0, 0]], [[float('nan'), 0, 0]], [[0, 0]], [[0, 0, 0]] * 17]),
                            ('grouped', [1, 'Yes']), ('finish', ['Delete', 'Undo']),
                            ('inputs', [[], ['Copy=Maybe'], ['_Delete'], ['90 _Delete'], ['w1,2'], ['1e999'], ['1,2,3,4'], ['.'], ['1'] * 33]),
                            ('extra', [True])]:
            for value in values:
                invalid = copy.deepcopy(operation)
                invalid[key] = value
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    validate(invalid)

    def test_private_scheme_is_required_before_launch(self):
        client = Mock(settings_scheme=None)
        with self.assertRaisesRegex(ValueError, 'private scheme'):
            capture(request(), client)
        client.run_rhino.assert_not_called()
        with (patch.dict(os.environ, {'DISPLAY': ':101', 'VIBOCEROS_ORACLE_HEADLESS': ':101'}),
              patch('tools.rhino_oracle.client._run_logged') as launch,
              self.assertRaisesRegex(OracleProtocolError, 'private Rhino settings scheme')):
            OracleClient(launcher='/bin/true').run_rhino(request())
        launch.assert_not_called()

    def test_raw_terminal_status_history_and_order_are_retained(self):
        observed = json.loads(Path(__file__).with_name('observations').joinpath('transform_copy.json').read_text())
        client = Mock(settings_scheme='VibocerosOracleTest')
        client.run_rhino.return_value = observed
        self.assertEqual(capture(request(), client), observed)
        rotate = next(row['value'] for row in observed['results'] if row['id'] == 'rotate-numbers-enter')
        self.assertFalse(rotate['succeeded'])
        terminal = next(event for event in rotate['events'] if event['name'] == 'Rotate')
        self.assertEqual(terminal['result'], 'Nothing')
        self.assertEqual(len(rotate['after']['objects']), 3)
        for mutation in ['missing_case', 'reordered', 'missing_undo', 'status', 'event', 'group']:
            invalid = copy.deepcopy(observed)
            value = invalid['results'][0]['value']
            if mutation == 'missing_case':
                invalid['results'].pop()
            elif mutation == 'reordered':
                invalid['results'][0], invalid['results'][1] = invalid['results'][1], invalid['results'][0]
            elif mutation == 'missing_undo':
                value['undo'] = None
            elif mutation == 'status':
                value['succeeded'] = 1
            elif mutation == 'event':
                value['events'] = []
            else:
                value['after']['objects'][0]['groups'] = [0]
            client.run_rhino.return_value = invalid
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                capture(request(), client)


if __name__ == '__main__':
    unittest.main()
