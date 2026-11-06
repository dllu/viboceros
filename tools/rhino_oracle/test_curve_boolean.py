"""Owned recipes and immutable native CurveBoolean producer provenance."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .curve_boolean_cases import pilot, holdouts, interactions, confirmations, weighted_deletion, validate
from .curve_boolean_subcurve_cases import pilot as subcurves, holdouts as subcurve_holdouts, edges as subcurve_edges

HERE = Path(__file__).resolve().parent


def observed(name):
    path = HERE / ('observations/' + name + '.json')
    raw = path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes())
    return json.loads(raw)


class CurveBooleanEvidenceTests(unittest.TestCase):
    def test_fixed_requests_and_every_frozen_producer(self):
        for family, recipes in [('pilot', pilot), ('holdouts', holdouts),
                                ('interactions', interactions), ('confirmations', confirmations),
                                ('weighted', weighted_deletion), ('subcurves', subcurves),
                                ('subcurve_holdouts', subcurve_holdouts), ('subcurve_edges', subcurve_edges),
                                ('subcurve_edge_receipt', subcurve_edges)]:
            name = 'curve_boolean_' + family
            request = (HERE / ('fixtures/' + name + '.json')).read_bytes()
            query = json.loads(request)
            self.assertEqual(query['operations'], recipes())
            for operation in query['operations']:
                validate(operation)
            result = observed(name)
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
        result = observed('curve_boolean_confirmations')
        row = next(r['value'] for r in result['results'] if r['id'].endswith('cancel_remembers'))
        self.assertFalse(row['success'])
        for option in ['DeleteInput=None', 'CombineRegions=No', 'Output=Surfaces']:
            self.assertIn(option, row['preference_history'])
        self.assertEqual(row['before'], row['after'])

    def test_temporary_parents_and_nested_cancellation_are_immutable(self):
        for family, recipes in [('subcurves', subcurves), ('subcurve_holdouts', subcurve_holdouts),
                                ('subcurve_edges', subcurve_edges)]:
            captured = observed('curve_boolean_' + family)
            for operation, row in zip(recipes(), captured['results'], strict=True):
                value = row['value']
                if operation.get('cancel_subcurve'):
                    self.assertFalse(value['success'])
                    self.assertEqual(value['before'], value['after'])
                    self.assertEqual(value['cancel_guard'][0]['action'], 'CancelRemainingCommand')
                    self.assertTrue(value['cancel_guard'][0]['prompt'].startswith('Select curves'))
                    continue
                self.assertTrue(value['success'])
                whole = {s['source'] for s in operation['selections'] if s['range'] is None}
                temporary = {s['source'] for s in operation['selections'] if s['range'] is not None} - whole
                for source in temporary:
                    original = next(o for o in value['before']['objects'] if o['source'] == source)
                    for phase in ['after', 'undo', 'redo']:
                        self.assertEqual(original, next(o for o in value[phase]['objects'] if o['source'] == source))
                self.assertEqual(value['after'], value['redo'])
                if 'surfaces' in operation:
                    guard, = value['edge_guard']
                    for key in ['acknowledged', 'frustum_verified', 'world_coordinates']:
                        self.assertIs(guard[key], True)
                    self.assertEqual(guard['source'], 1)
                    self.assertEqual(guard['edge'], 0)
                    self.assertEqual(guard['marker'], '@component-click:' + operation['id'] + ':0:sub')

    def test_edge_confirmation_binds_all_input_producers_and_matches_original_snapshots(self):
        original = observed('curve_boolean_subcurve_edges')
        confirmed = observed('curve_boolean_subcurve_edge_receipt')
        self.assertEqual(original['engine_version'], confirmed['engine_version'])
        self.assertEqual(original['provenance']['request_sha256'], confirmed['provenance']['request_sha256'])
        self.assertTrue({'curve_boolean_edge_input.py', 'owned_brep_command.py', 'client.py',
                         'component_picking.py', 'hole_picking.py', 'group_picking.py'} <= confirmed['provenance']['sources'].keys())
        for a, b in zip(original['results'], confirmed['results'], strict=True):
            self.assertEqual(a['id'], b['id'])
            for key in ['success', 'before', 'after', 'undo', 'redo']:
                self.assertEqual(a['value'][key], b['value'][key])


if __name__ == '__main__':
    unittest.main()
