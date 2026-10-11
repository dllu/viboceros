"""Fixed curved recipes and public command/driver evidence."""
import json
import math
from pathlib import Path
import tempfile
from unittest import TestCase, mock

from .boolean_two_curved_probe import request, validate_request, SPECS, run
from .boolean_two_input import BooleanTwoPicker
from .client import OracleProtocolError

ROOT = Path(__file__).resolve().parents[2]


class CurvedBooleanTwoTests(TestCase):
    def test_fixed_bounded_recipes(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/boolean_two_curved.json').read_text()))
        for change in [dict(case='_Exit'), dict(case=[]), dict(id='x\n_Exit'), dict(extra=True), dict(op='script')]:
            operation = dict(q['operations'][0], **change)
            with self.subTest(change=change):
                with self.assertRaises(ValueError):
                    validate_request(dict(q, operations=[operation]))
                with self.assertRaises(ValueError):
                    run(operation, {})

    def test_shared_driver_ignores_completed_cases_and_rejects_foreign_markers(self):
        picker = BooleanTwoPicker(request(), validate_request, SPECS)
        with tempfile.TemporaryDirectory() as directory:
            picker.job = Path(directory)
            (picker.job/'worker-progress.log').write_text('PICK_DONE @boolean-two:boolean_two_curved_cancel\n')
            with mock.patch('tools.rhino_oracle.boolean_two_input.subprocess.run') as send:
                self.assertTrue(picker.send_input('@boolean-two:boolean_two_curved_cancel', '100', '100', 'owned'))
                with self.assertRaises(OracleProtocolError):
                    picker.send_input('@foreign', '100', '100', 'owned')
                send.assert_not_called()

    def test_public_cycles_branch_metadata_identity_and_history(self):
        capture = json.loads((ROOT/'tools/rhino_oracle/observations/boolean_two_curved.json').read_text())
        self.assertEqual(capture['engine_version'], '8.32.26160.13001')
        self.assertEqual([r['id'] for r in capture['results']], [o['id'] for o in request()['operations']])
        expected = [72.+32.*math.pi, 8.*math.pi, 32.*math.pi, 72.-8.*math.pi, 72.+24.*math.pi]
        for row in capture['results']:
            value = row['value']
            before, after = value['before'], value['command']['after_script']
            self.assertTrue(all(o['valid'] and o['solid'] for o in before+after))
            self.assertNotIn('Unknown command:', value['command']['history'])
            self.assertFalse(any(o['selected'] for o in after))
            if value['cancel']:
                self.assertFalse(value['command']['success'])
                self.assertEqual(after, [dict(o, selected=False) for o in before])
                continue
            self.assertTrue(value['command']['success'])
            self.assertEqual(value['undo']['after_script'], [dict(o, selected=False) for o in before])
            self.assertEqual(value['redo']['after_script'], after)
            outputs = after[2:] if not value['delete'] else after
            self.assertEqual(outputs[0]['source'], 0 if value['delete'] else None)
            self.assertAlmostEqual(sum(o['volume'] for o in outputs), expected[value['cycles'] % 5], delta=2e-6)
            for output in outputs:
                for key in ['name', 'layer', 'color', 'groups', 'attribute_text']:
                    self.assertEqual(output[key], before[0][key])
                cleared = value['cycles'] % 5 == 2 or (value['cycles'] % 5 == 4 and output is not outputs[0])
                self.assertEqual(output['geometry_text'], None if cleared else 'geometry-0')

    def test_public_box_split_order_disproves_universal_reversal(self):
        from .boolean_two_probe import validate_request as validate_boxes
        fixture = json.loads((ROOT/'tools/rhino_oracle/fixtures/boolean_two_split.json').read_text())
        validate_boxes(fixture)
        capture = json.loads((ROOT/'tools/rhino_oracle/observations/boolean_two_split.json').read_text())
        self.assertEqual([r['id'] for r in capture['results']], [o['id'] for o in fixture['operations']])
        for row in capture['results']:
            value = row['value']
            self.assertTrue(value['command']['success'])
            after = value['command']['after_script']
            pieces = after if value['cycles'] == 2 else after[1:]
            self.assertEqual([p['geometry_text'] for p in pieces], [None, None])
            for part, z in zip(pieces, [2., 8.]):
                self.assertAlmostEqual(part['volume'], 64.)
                self.assertAlmostEqual(part['centroid'][2], z)
            self.assertEqual(after[0]['source'], 0)
            self.assertEqual(value['undo']['after_script'], [dict(o, selected=False) for o in value['before']])
            self.assertEqual(value['redo']['after_script'], after)
