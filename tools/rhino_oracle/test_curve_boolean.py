"""Owned recipes and immutable native CurveBoolean producer provenance."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .curve_boolean_cases import pilot, holdouts, interactions, confirmations, weighted_deletion, validate

HERE = Path(__file__).resolve().parent


class CurveBooleanEvidenceTests(unittest.TestCase):
    def test_fixed_requests_and_every_frozen_producer(self):
        for family, recipes in [('pilot', pilot), ('holdouts', holdouts),
                                ('interactions', interactions), ('confirmations', confirmations),
                                ('weighted', weighted_deletion)]:
            name = 'curve_boolean_' + family
            request = (HERE / ('fixtures/' + name + '.json')).read_bytes()
            query = json.loads(request)
            self.assertEqual(query['operations'], recipes())
            for operation in query['operations']:
                validate(operation)
            result = json.loads((HERE / ('observations/' + name + '.json')).read_text())
            self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in result['results']])
            provenance = result['provenance']
            self.assertEqual(provenance['request_sha256'], hashlib.sha256(request).hexdigest())
            for filename, digest in provenance['sources'].items():
                archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
                self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), digest, filename)

    def test_mutated_owner_recipes_are_rejected(self):
        source = pilot()[5]
        for mutate in [lambda r: r.update(id='foreign'), lambda r: r.update(extra=True),
                       lambda r: r['curves'][0]['control_points'][0].update(point=[True, 0., 0.]),
                       lambda r: r['points'][0].__setitem__(0, True),
                       lambda r: r['options'].update(delete_input='Everything')]:
            operation = copy.deepcopy(source)
            mutate(operation)
            with self.assertRaisesRegex(ValueError, 'fixed owned'):
                validate(operation)

    def test_cancelled_options_remain_visible_in_native_next_prompt(self):
        result = json.loads((HERE / 'observations/curve_boolean_confirmations.json').read_text())
        row = next(r['value'] for r in result['results'] if r['id'].endswith('cancel_remembers'))
        self.assertFalse(row['success'])
        for option in ['DeleteInput=None', 'CombineRegions=No', 'Output=Surfaces']:
            self.assertIn(option, row['preference_history'])
        self.assertEqual(row['before'], row['after'])


if __name__ == '__main__':
    unittest.main()
