"""Curved inset sources, immutable producers, fit modes and actual history."""
import gzip
import hashlib
import json
import unittest
from .capture_family import HERE
from .generate_inset_surface import request
from .inset_planar_curves import cases, validate
from .inset_planar_curves_command import cases as command_cases, validate as command_validate


def native(family):
    return json.loads(gzip.decompress((HERE / ('observations/' + family + '.json.gz')).read_bytes()))


class CurvedInsetTests(unittest.TestCase):
    def test_frozen_requests_and_producers(self):
        for family, operations in [('inset_planar_curves',cases()), ('inset_planar_curves_command',command_cases())]:
            query = request(); query['operations'] = operations
            path = HERE / ('fixtures/' + family + '.json')
            data = native(family)
            self.assertEqual(json.loads(path.read_bytes()),query)
            self.assertEqual([r['id'] for r in data['results']], [op['id'] for op in operations])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for sha in data['provenance']['sources'].values():
                contents = gzip.decompress((HERE / ('observations/source_snapshots/' + sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(contents).hexdigest(),sha)
            for row in data['results']: self.assertNotIn('error',row)

    def test_loose_and_tight_results_are_distinguished(self):
        rows = {r['id']:r['value'] for r in native('inset_planar_curves')['results']}
        for row in rows.values():
            self.assertTrue(row['source_unchanged'])
            self.assertTrue(row['before']['valid'])
            self.assertTrue(row['output']['valid'])
        self.assertEqual(len(rows['ellipse_tight_ring']['output']['geometry']['edges']),2)
        self.assertEqual(len(rows['ellipse_loose_ring']['output']['geometry']['edges']),5)
        self.assertEqual(len(rows['stadium_tight_ring']['output']['geometry']['faces']),2)
        self.assertEqual(len(rows['stadium_tight_crease']['output']['geometry']['faces']),5)

    def test_joined_command_selection_metadata_and_history(self):
        for row in native('inset_planar_curves_command')['results']:
            value = row['value']
            self.assertEqual(value['results'],['Success'])
            self.assertNotIn('Unknown command:',value['history'])
            self.assertEqual(value['before'][0]['brep'],value['undo'][0]['brep'])
            self.assertEqual(len(value['after']),1)
            self.assertEqual(value['after'][0]['brep'],value['redo'][0]['brep'])
            for key in ('before','after','undo','redo'):
                obj = value[key][0]
                self.assertEqual(obj['source'],0)
                self.assertEqual(obj['groups'],[0])
                self.assertEqual(obj['color'],[17,43,91])
                self.assertEqual(obj['name'],'Input')
            self.assertTrue(value['after'][0]['selected'])
            self.assertFalse(value['after_script'][0]['selected'])
            self.assertEqual(value['after'][0]['selected_faces'],[])

    def test_unowned_and_malformed_recipes(self):
        for check, op in [(validate,cases()[0]), (command_validate,command_cases()[0])]:
            check(op)
            for changes in ({'id':'unowned'},{'scene':'unknown'},{'crease_corners':1},{'extra':True}):
                with self.assertRaises(ValueError): check(dict(op,**changes))
        with self.assertRaises(ValueError): validate(dict(cases()[0],distance=True))


if __name__ == '__main__': unittest.main()
