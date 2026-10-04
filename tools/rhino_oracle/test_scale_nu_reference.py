"""Prescribed cursor rays and distinct keyboard/macro reference evidence."""
import copy
import json
import math
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

from .client import OracleClient, OracleError, OracleProtocolError
from .move_normal_probe import validate_camera
from .scale_nu_reference_input import ScaleNuReferencePicker
from .scale_nu_reference_probe import recipe, request, run, validate_request
from .translation_input import validate_frame

ROOT = Path(__file__).parent


def axis_coordinate(frame, axis):
    """Closest point of a captured viewing ray to a prescribed world axis."""
    a, b = frame['ray']
    vector = [y-x for x, y in zip(a, b)]
    length = math.sqrt(sum(x*x for x in vector))
    direction = [x/length for x in vector]
    projected = sum(x*y for x, y in zip(a, direction))
    return (a[axis]-projected*direction[axis])/(1.-direction[axis]**2)


class ScaleNuReferenceTests(unittest.TestCase):
    def test_closed_bounded_recipes(self):
        q = request()
        self.assertEqual(q, json.loads((ROOT/'fixtures/scale_nu_reference.json').read_text()))
        validate_request(q)
        self.assertEqual(len(q['operations']), 32)
        case = q['operations'][0]
        for changes in (dict(id='x\n_Delete'), dict(id='λ'), dict(id=[]), dict(op='Delete'),
                dict(script='_Delete'), dict(axis=True), dict(axis=3), dict(view=[]),
                dict(view='Delete'), dict(aim=[]), dict(aim='custom'), dict(finish=[]),
                dict(finish='Return'), dict(reference=[]), dict(reference='World')):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate_request(dict(protocol_version=1, operations=[dict(case, **changes)]))
        for q in (None, [], dict(protocol_version=True, operations=[case]),
                dict(protocol_version=1, iterations=True, operations=[case]),
                dict(protocol_version=1, iterations=2, operations=[case]),
                dict(protocol_version=1, operations=[]),
                dict(protocol_version=1, operations=[case, case]),
                dict(protocol_version=1, operations=[dict(case,id=str(i)) for i in range(33)])):
            with self.subTest(request=q), self.assertRaises(ValueError):
                validate_request(q)

    def test_idle_owned_document_and_private_launch_guards(self):
        q = request()
        rhino = Mock()
        rhino.Commands.Command.InCommand.return_value = False
        rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError, 'empty owned document'):
            run(q['operations'][0], {'Rhino': rhino, 'System': Mock()})
        rhino.Commands.Command.InCommand.return_value = True
        with self.assertRaisesRegex(ValueError, 'idle execution'):
            run(q['operations'][0], {'Rhino': rhino, 'System': Mock()})
        for scheme, iterations, display, marker in ((None,1,':301',':301'),
                ('VibocerosOracleScaleNURef',True,':301',':301'),
                ('VibocerosOracleScaleNURef',2,':301',':301'),
                ('VibocerosOracleScaleNURef',1,':1',None),
                ('VibocerosOracleScaleNURef',1,':301',':302')):
            env = {'DISPLAY': display}
            if marker: env['VIBOCEROS_ORACLE_HEADLESS'] = marker
            with patch.dict(os.environ, env, clear=True), patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError, OracleProtocolError, ValueError)):
                    OracleClient(settings_scheme=scheme).run_rhino(dict(q, iterations=iterations), 1)
                launch.assert_not_called()

    def test_picker_waits_for_native_motion_and_scopes_input_to_owned_window(self):
        op = request()['operations'][0]
        marker = '@scale-nu-reference:'+op['id']
        picker = ScaleNuReferencePicker(dict(protocol_version=1, operations=[op]))
        with tempfile.TemporaryDirectory() as folder, patch('tools.rhino_oracle.scale_nu_reference_input.subprocess.run') as send, patch('tools.rhino_oracle.scale_nu_reference_input.time.monotonic', return_value=0.):
            picker.job = Path(folder)
            self.assertFalse(picker.send_input(marker, '100', '120', 'owned'))
            self.assertEqual(send.call_args.args[0], ['xdotool','windowactivate','--sync','owned','mousemove','101','120','mousemove','100','120'])
            self.assertFalse(picker.send_input(marker, '100', '120', 'owned'))
            self.assertEqual(send.call_count, 1)
            with self.assertRaises(OracleProtocolError): picker.send_input(marker, '100', '120', 'foreign')
            with self.assertRaises(OracleProtocolError): picker.send_input('@unexpected', '100', '120', 'owned')
            ready = picker.job/('scale-nu-reference-ready-'+op['id']+'.json')
            ready.write_text(json.dumps('foreign-marker'))
            with self.assertRaises(OracleProtocolError): picker.send_input(marker, '100', '120', 'owned')
            ready.write_text(json.dumps(marker))
            with patch('tools.rhino_oracle.scale_nu_reference_input.time.monotonic', return_value=.3):
                self.assertTrue(picker.send_input(marker, '100', '120', 'owned'))
            self.assertEqual(send.call_args.args[0], ['xdotool','windowactivate','--sync','owned','click','1'])

    def test_keyboard_and_script_calibration_are_distinct_prescribed_inputs(self):
        for index in (1,20):
            op = request()['operations'][index]
            marker = '@scale-nu-reference:'+op['id']
            picker = ScaleNuReferencePicker(dict(protocol_version=1, operations=[op]))
            picker.moved[marker] = ('owned','100','120',0.)
            with tempfile.TemporaryDirectory() as folder, patch('tools.rhino_oracle.scale_nu_reference_input.subprocess.run') as send, patch('tools.rhino_oracle.scale_nu_reference_input.time.monotonic', return_value=.3), patch('tools.rhino_oracle.scale_nu_reference_input.time.sleep') as settle:
                picker.job = Path(folder)
                (picker.job/('scale-nu-reference-ready-'+op['id']+'.json')).write_text(json.dumps(marker))
                self.assertTrue(picker.send_input(marker, '100', '120', 'owned'))
                if op['finish'] == 'Typed':
                    self.assertEqual(send.call_args_list[0].args[0], ['xdotool','windowactivate','--sync','owned','type','--clearmodifiers','--delay','30',recipe(op)['typed_target']])
                    self.assertEqual(send.call_args.args[0], ['xdotool','windowactivate','--sync','owned','key','--clearmodifiers','Return'])
                    settle.assert_called_once_with(.25)
                else:
                    self.assertEqual(send.call_args.args[0][-2:], ['click','1'])
                    settle.assert_not_called()

    def test_acknowledgements_and_response_order_preserve_raw_outputs(self):
        q = request()
        picker = ScaleNuReferencePicker(q)
        response = dict(results=[dict(id=op['id'],value={'raw':True}) for op in q['operations']])
        original = copy.deepcopy(response)
        with self.assertRaises(OracleProtocolError): picker.record_diagnostics(response)
        picker.seen = set(picker.cases)
        picker.record_diagnostics(response)
        self.assertEqual(response, original)
        for rows in (None, [], list(reversed(response['results'])), [{}], [None]):
            with self.assertRaises(OracleProtocolError): picker.record_diagnostics(dict(results=rows))

    def test_native_cursor_keyboard_and_history(self):
        q = request()
        observed = json.loads((ROOT/'observations/scale_nu_reference.json').read_text())
        self.assertEqual([op['id'] for op in q['operations']], [row['id'] for row in observed['results']])
        for op, row in zip(q['operations'], observed['results']):
            v = row['value']
            label = op['id']
            self.assertTrue(v['success'], label)
            self.assertEqual(v['before'], v['undo'], label)
            self.assertEqual(v['after'], v['after_script'], label)
            self.assertEqual(v['after'], v['redo'], label)
            self.assertEqual(v['before'], v['pending']['objects'], label)
            self.assertEqual(v['recipe'], recipe(op), label)
            validate_frame(v['pending']['frame'])
            validate_camera(v['pending']['camera'], v['pending']['frame'])
            if op['finish'] == 'Scripted': continue
            axis = op['axis']
            distance = 6. if op['finish'] == 'Typed' else abs(axis_coordinate(v['pending']['frame'], axis))
            factor = distance/abs(recipe(op)['reference'][axis])
            expected = [2.,3.,4.]
            expected[axis] *= factor
            for a, b in zip(v['after'][0]['point'], expected):
                self.assertAlmostEqual(a, b, delta=1e-9, msg=label)
            self.assertEqual(v['pending']['phase'], 'second_reference')
            self.assertIsNone(v['calibration'])

    def test_scripted_targets_follow_prior_cursor_calibration(self):
        observed = json.loads((ROOT/'observations/scale_nu_reference.json').read_text())
        cases = [(op, row['value']) for op, row in zip(request()['operations'], observed['results'])
                 if op['finish'] == 'Scripted']
        self.assertEqual(len(cases), 12)
        for op, v in cases:
            axis = op['axis']
            label = op['id']
            self.assertEqual(v['pending']['phase'], 'cursor_calibration', label)
            self.assertEqual(v['calibration']['result'], 'Point', label)
            cursor = axis_coordinate(v['pending']['frame'], axis)
            calibration = [0.,0.,0.]
            calibration[axis] = cursor
            for a, b in zip(v['calibration']['point'], calibration):
                self.assertAlmostEqual(a, b, delta=1e-9, msg=label)
            expected = [2.,3.,4.]
            expected[axis] *= abs(cursor)/abs(recipe(op)['reference'][axis])
            for a, b in zip(v['after'][0]['point'], expected):
                self.assertAlmostEqual(a, b, delta=1e-9, msg=label)
            # These are retained discrepancies, never converted to successful
            # comparisons with a script-derived point or rewritten geometry.
            typed = [2.,3.,4.]
            typed[axis] *= 6./abs(recipe(op)['reference'][axis])
            self.assertGreater(abs(v['after'][0]['point'][axis]-typed[axis]), 1e-6, label)
            self.assertIn(recipe(op)['typed_target'], v['history'], label)

    def test_provenance(self):
        provenance = json.loads((ROOT.parents[1]/'docs/scale-nu-reference-provenance.json').read_text())
        self.assertTrue(provenance['private_xvfb'])
        self.assertFalse(provenance['full_native_parity'])
        self.assertEqual(provenance['native_recipes'], 32)
        self.assertEqual(provenance['positive_application_replays'], 20)
        self.assertEqual(provenance['scripted_cursor_diagnostics'], 12)
        import hashlib
        for path, expected in provenance['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT.parents[1]/path).read_bytes()).hexdigest(), expected, path)


if __name__ == '__main__': unittest.main()
