"""Repeated transforms require bounded inputs, private settings, and raw events."""
import copy
import json
import os
from pathlib import Path
import unittest
from unittest.mock import Mock, patch

from .client import OracleClient, OracleProtocolError
from .transform_copy_capture import capture, validate_request
from .transform_copy_cases import request, script_request, center_request, identity_request, default_request, mirror_request, mirror_enter_request, mirror_object_request, sources_request, sources_identity_request
from .transform_copy_probe import validate
from .translation_cases import request as translation_request, edges_request as translation_edges_request, mouse_request as translation_mouse_request


class TransformCopyTests(unittest.TestCase):
    def test_script_and_center_workflows_prescribe_inputs_before_measurement(self):
        for name, factory, count in [('transform_copy_script', script_request, 56),
                                     ('transform_copy_center', center_request, 18),
                                     ('transform_copy_identity', identity_request, 64),
                                     ('transform_copy_default', default_request, 80),
                                     ('mirror_planes', mirror_request, 64),
                                     ('mirror_enter', mirror_enter_request, 5),
                                     ('mirror_object', mirror_object_request, 70),
                                     ('transform_sources', sources_request, 61),
                                     ('transform_sources_identity', sources_identity_request, 12),
                                     ('translation', translation_request, 110),
                                     ('translation_edges', translation_edges_request, 36),
                                     ('translation_mouse', translation_mouse_request, 18)]:
            saved = json.loads(Path(__file__).with_name('fixtures').joinpath(name+'.json').read_text())
            self.assertEqual(saved, factory())
            self.assertEqual(len(saved['operations']), count)
            validate_request(saved)
            observed = json.loads(Path(__file__).with_name('observations').joinpath(name+'.json').read_text())
            client = Mock(settings_scheme='VibocerosOracleTest', run_rhino=Mock(return_value=observed))
            self.assertEqual(capture(saved, client), observed)

    def test_translation_clicks_and_calibration_are_bounded(self):
        for key, values in [('view',['Top','Delete']),('aim',[[0,0,float('nan')]]),('bounds',[[[0,0,0],[0,0,0]]]),('offset',[[True,0],[0,33]]),('extra',[True])]:
            for value in values:
                invalid = translation_mouse_request()
                invalid['operations'][0]['mouse_target'][key] = value
                client = Mock(settings_scheme='VibocerosOracleTest')
                with self.subTest(key=key,value=value), self.assertRaises(ValueError): capture(invalid, client)
                client.run_rhino.assert_not_called()
        request = translation_mouse_request()
        observed = json.loads(Path(__file__).with_name('observations').joinpath('translation_mouse.json').read_text())
        for mutation in ('missing','zero-ray','nonfinite','outside'):
            invalid = copy.deepcopy(observed); frame = invalid['results'][0]['value']['frame']
            if mutation == 'missing': del frame['ray']
            elif mutation == 'zero-ray': frame['ray'][1] = frame['ray'][0]
            elif mutation == 'nonfinite': frame['world_to_screen'][0][0] = float('nan')
            else: frame['click_client'] = frame['size']
            client = Mock(settings_scheme='VibocerosOracleTest',run_rhino=Mock(return_value=invalid))
            with self.subTest(mutation=mutation), self.assertRaises(OracleProtocolError if mutation == 'nonfinite' else ValueError): capture(request,client)

    def test_source_selection_is_bounded_to_owned_ids_and_named_inputs(self):
        for steps in [[], [True], [-1], [4], ['Delete'], ['0 _Delete'], [None], [0]*33, 'SelAll']:
            invalid = sources_request()
            invalid['operations'][0]['source_selection'] = steps
            client = Mock(settings_scheme='VibocerosOracleTest')
            with self.subTest(steps=steps), self.assertRaises(ValueError):
                capture(invalid, client)
            client.run_rhino.assert_not_called()
        invalid = sources_request()
        invalid['operations'][0]['selected'] = [0]
        with self.assertRaises(ValueError):
            validate_request(invalid)

    def test_translation_tokens_are_command_specific_and_bounded(self):
        for command, tokens in [('Move', ['InPlace', 'UseLastDistance=Yes', 'Vertical=Yes']),
                                ('Scale', ['Vertical', 'InPlace', 'FromLastPoint=No']),
                                ('Copy', ['FromLastPoint=Maybe', 'UseLastDistance=Yes _Delete', 'Normal _Pause'])]:
            for token in tokens:
                invalid = translation_request()['operations'][0]
                invalid['command'] = command
                invalid['inputs'] = [token]
                with self.subTest(command=command, token=token), self.assertRaises(ValueError):
                    validate(invalid)

    def test_mirror_options_are_bounded_and_witnesses_are_affinely_independent(self):
        operation = mirror_request()['operations'][0]
        a, b, c, d = operation['sources'][:4]
        b, c, d = [[v-u for u, v in zip(a, point)] for point in [b, c, d]]
        determinant = sum(b[i] * (c[(i+1)%3]*d[(i+2)%3] - c[(i+2)%3]*d[(i+1)%3]) for i in range(3))
        self.assertNotEqual(determinant, 0.)
        for token in ['3Point', 'XAxis', 'YAxis', 'ZAxis', 'Object']:
            invalid = copy.deepcopy(operation)
            invalid['command'] = 'Scale'
            invalid['inputs'] = [token]
            with self.subTest(token=token), self.assertRaises(ValueError):
                validate(invalid)
        invalid = copy.deepcopy(operation)
        invalid['inputs'] = ['Target']
        with self.assertRaises(ValueError):
            validate(invalid)

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

    def test_owned_mirror_target_inputs_are_bounded_before_launch(self):
        operation=mirror_object_request()['operations'][0]
        for key,values in [('kind',['Delete',None]),('pick',['unowned',None]),('view',['Perspective','Delete']),
                           ('face',[True,-1,6]),('corners',[[[float('nan'),0,0]]*4,[[0,0,0]]*3]),
                           ('menu',['Second','Delete']),('fraction',[[True,0],[.5,2]]),('modifiers',['alt']),('extra',[True])]:
            for value in values:
                invalid=copy.deepcopy(operation);invalid['mirror_target'][key]=value
                with self.subTest(key=key,value=value),self.assertRaises(ValueError):validate(invalid)
        for inputs in [['Target','Target'],['Target _Delete']]:
            invalid=copy.deepcopy(operation);invalid['inputs']=inputs
            with self.assertRaises(ValueError):validate(invalid)

    def test_plane_target_geometry_and_selection_must_remain_unchanged(self):
        request=mirror_object_request()
        observed=json.loads(Path(__file__).with_name('observations').joinpath('mirror_object.json').read_text())
        for mutation in ['selected','definition','missing','missing_geometry','bounds']:
            invalid=copy.deepcopy(observed);target=invalid['results'][0]['value']['target']['after']
            if mutation=='selected':target['selected']=True
            elif mutation=='definition':target['definition']['faces'][0]['definition']['control_points'][0]['point'][0]+=1
            elif mutation=='missing':del invalid['results'][0]['value']['target']
            elif mutation=='missing_geometry':
                del target['definition'];del invalid['results'][0]['value']['target']['before']['definition']
            else:target['bounds'][0][0]+=1e-7
            client=Mock(settings_scheme='VibocerosOracleTest',run_rhino=Mock(return_value=invalid))
            if mutation=='bounds':self.assertEqual(capture(request,client),invalid)
            else:
                with self.subTest(mutation=mutation),self.assertRaises(ValueError):capture(request,client)

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
