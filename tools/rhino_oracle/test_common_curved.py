import json
from pathlib import Path
import unittest

from .common_curved_probe import request, validate_request

ROOT = Path(__file__).resolve().parents[2]


class CurvedCommonTests(unittest.TestCase):
    def test_fixed_recipe_validation(self):
        validate_request(request())
        for changes in [dict(case='unknown'), dict(id=''), dict(op='arbitrary')]:
            operation = dict(request()['operations'][0])
            operation.update(changes)
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate_request(dict(protocol_version=1, operations=[operation]))

    def test_public_command_outcomes_and_history(self):
        capture = json.loads((ROOT / 'tools/rhino_oracle/observations/common_curved.json').read_text())
        self.assertEqual(capture['engine_version'], '8.32.26160.13001')
        self.assertEqual([r['id'] for r in capture['results']], [op['id'] for op in request()['operations']])
        for row in capture['results']:
            value = row['value']
            events = [e for e in value['command']['events'] if e['name'] == 'BooleanIntersection']
            expected = 'Success' if row['id'] == 'curved_common_crossing' else 'Nothing'
            self.assertEqual([e['result'] for e in events], [expected])
            self.assertNotIn('Unknown command:', value['command']['history'])
            self.assertTrue(all(g['valid'] and g['solid'] for g in value['before']))
            if expected == 'Success':
                self.assertEqual(len(value['command']['after']), 1)
                self.assertEqual(len(value['undo']['after']), 2)
                self.assertEqual(len(value['redo']['after']), 1)
            else:
                self.assertEqual(len(value['command']['after']), 2)
