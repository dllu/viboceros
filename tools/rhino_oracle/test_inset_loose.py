"""Frozen Loose basis controls, real commands and option lifetime evidence."""
import gzip
import hashlib
import json
import unittest
from .capture_family import HERE
from .generate_inset_surface import request
from .inset_loose_controls import cases, validate
from .inset_loose_command import cases as commands, validate as command_validate, PREFERENCES


def native(family):
    return json.loads(gzip.decompress((HERE / ('observations/' + family + '.json.gz')).read_bytes()))


class LooseInsetTests(unittest.TestCase):
    def test_owned_requests_and_immutable_producers(self):
        for family, operations in [('inset_loose_controls', cases()),
                                   ('inset_loose_command', commands()),
                                   ('inset_loose_preferences', [PREFERENCES])]:
            query = request(); query['operations'] = operations
            path = HERE / ('fixtures/' + family + '.json')
            evidence = native(family)
            self.assertEqual(json.loads(path.read_bytes()), query)
            self.assertEqual([r['id'] for r in evidence['results']], [op['id'] for op in operations])
            self.assertEqual(evidence['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            for sha in evidence['provenance']['sources'].values():
                source = gzip.decompress((HERE / ('observations/source_snapshots/' + sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(source).hexdigest(), sha)
            for result in evidence['results']: self.assertNotIn('error', result)

    def test_basis_distance_and_fixed_kink_angle_controls(self):
        rows = {r['id']: r['value'] for r in native('inset_loose_controls')['results']}
        for row in rows.values():
            self.assertTrue(row['source_unchanged'])
            self.assertTrue(row['output']['valid'])
        edges = lambda key: len(rows[key]['output']['geometry']['edges'])
        for crease in (False, True):
            self.assertEqual(edges('ellipse_1e-08_' + str(crease)), 5)
            self.assertEqual(edges('ellipse_0.1_' + str(crease)), 5)
            for distance, count in [(0.05, 2), (0.1, 3), (0.4, 5)]:
                self.assertEqual(edges('ellipse_distance_' + str(distance) + '_' + str(crease)), count)
            for degree, count in [(3, 3), (7, 2)]:
                self.assertEqual(edges('ellipse_degree_' + str(degree) + '_' + str(crease)), count)

    def test_commands_history_and_invocation_local_loose_setting(self):
        for result in native('inset_loose_command')['results']:
            row = result['value']
            self.assertEqual(row['results'], ['Success'])
            self.assertNotIn('Unknown command:', row['history'])
            self.assertEqual(row['before'][0]['brep'], row['undo'][0]['brep'])
            self.assertEqual(row['after'][0]['brep'], row['redo'][0]['brep'])
            self.assertTrue(row['after'][0]['selected'])
            self.assertFalse(row['after_script'][0]['selected'])
        steps = native('inset_loose_preferences')['results'][0]['value']['steps']
        for index, step in enumerate(steps):
            row = step['value']
            self.assertIn('Loose=Off', row['history'].splitlines()[1])
            self.assertIn('CreaseCorners=No', row['history'].splitlines()[1])
            if index in (2, 5):
                self.assertEqual(row['results'], ['Cancel'])
                self.assertEqual(row['before'][0]['brep'], row['after'][0]['brep'])
            else: self.assertEqual(row['results'], ['Success'])

    def test_unowned_malformed_and_boolean_recipes_decline(self):
        for check, op in [(validate, cases()[0]), (command_validate, commands()[0]),
                          (command_validate, PREFERENCES)]:
            check(op)
            for changes in ({'id': 'unknown'}, {'extra': True}, {'loose': 1}, {'scene': 'unowned'}):
                with self.assertRaises(ValueError): check(dict(op, **changes))


if __name__ == '__main__': unittest.main()
