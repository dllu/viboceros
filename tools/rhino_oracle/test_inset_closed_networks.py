"""Frozen closed-edge Inset crossings, adjacent faces and source seam controls."""
import gzip
import hashlib
import json
import unittest

from .capture_family import HERE
from .generate_inset_surface import request
from . import inset_closed_network_probe as adjacent
from . import inset_closed_seams_probe as seams
from .test_inset_continuity import native
from .test_inset_fans import topology


FAMILIES = (
    ('inset_closed_network', adjacent.cases, adjacent.validate),
    ('inset_closed_network_faces', adjacent.face_cases, adjacent.validate),
    ('inset_closed_seams', seams.cases, seams.validate),
)


class InsetClosedNetworksTests(unittest.TestCase):
    def test_owned_recipes_and_immutable_provenance(self):
        total = 0
        for family, cases, validate in FAMILIES:
            operations = cases()
            query = request(); query['operations'] = operations
            path = HERE / ('fixtures/' + family + '.json')
            data = native(family)
            self.assertEqual(json.loads(path.read_bytes()), query)
            self.assertEqual(data['engine'], 'rhino')
            self.assertEqual(data['engine_version'], '8.32.26160.13001')
            self.assertEqual([r['id'] for r in data['results']], [op['id'] for op in operations])
            self.assertEqual(data['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            for sha in data['provenance']['sources'].values():
                frozen = gzip.decompress((HERE / ('observations/source_snapshots/' + sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(frozen).hexdigest(), sha)
            for op, row in zip(operations, data['results']):
                validate(op)
                self.assertNotIn('error', row)
                for changes in ({'id': 'unowned'}, {'extra': True}, {'distance': True},
                                {'distance': float('nan')}, {'crease_corners': 1}):
                    with self.assertRaises(ValueError): validate(dict(op, **changes))
            total += len(operations)
        self.assertEqual(total, 14)

    def test_connectors_partition_the_unselected_face(self):
        for family in ('inset_closed_network', 'inset_closed_network_faces'):
            value = native(family)['results'][0]['value']
            self.assertTrue(value['source_unchanged'])
            self.assertTrue(value['output']['valid'])
            self.assertEqual(len(value['before']['geometry']['faces']), 2)
            self.assertEqual(topology(value), (9, 15, 9))

    def test_commands_require_component_selection_on_the_adjacent_brep(self):
        for row in native('inset_closed_network')['results'][1:]:
            value = row['value']
            self.assertEqual(value['results'], ['Cancel'])
            self.assertEqual(len(value['after']), 1)
            self.assertEqual(value['before'][0]['brep'], value['after'][0]['brep'])
        rows = native('inset_closed_network_faces')['results'][1:]
        self.assertEqual([len(r['value']['after']) for r in rows], [7, 6, 1])
        for row in rows:
            value = row['value']
            self.assertEqual(value['results'], ['Success'])
            self.assertNotIn('Unknown command:', value['history'])
            self.assertEqual(value['before'][0]['brep'], value['undo'][0]['brep'])
            self.assertEqual(value['after_script'], value['redo'])
        self.assertEqual(sorted(len(x['brep']['faces']) for x in rows[1]['value']['after']),
                         [1, 1, 1, 2, 2, 2])

    def test_straight_curved_and_g2_seams_have_distinct_topology(self):
        counts = [(6, 14, 10), (3, 4, 4), (6, 13, 9),
                  (3, 4, 4), (3, 4, 4), (4, 8, 6)]
        for row, expected in zip(native('inset_closed_seams')['results'], counts):
            value = row['value']
            self.assertTrue(value['source_unchanged'])
            self.assertTrue(value['output']['valid'])
            self.assertEqual(topology(value), expected, row['id'])


if __name__ == '__main__': unittest.main()
