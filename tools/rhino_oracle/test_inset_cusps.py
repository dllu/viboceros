"""Immutable curvature-limit evidence from SDK and real Inset commands."""
import gzip
import hashlib
import json
import math
import unittest
from .capture_family import HERE
from .generate_inset_surface import request
from .inset_cusp_probe import cases, validate


def native(family):
    return json.loads(gzip.decompress((HERE / ('observations/' + family + '.json.gz')).read_bytes()))


class InsetCuspTests(unittest.TestCase):
    def test_owned_requests_and_content_addressed_producers(self):
        for sdk in (False, True):
            family = 'inset_cusp_sdk' if sdk else 'inset_cusp_command'
            query = request(); query['operations'] = cases(sdk)
            path = HERE / ('fixtures/' + family + '.json'); data = native(family)
            self.assertEqual(json.loads(path.read_bytes()), query)
            self.assertEqual([r['id'] for r in data['results']], [op['id'] for op in cases(sdk)])
            self.assertEqual(data['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            for sha in data['provenance']['sources'].values():
                source = gzip.decompress((HERE / ('observations/source_snapshots/' + sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(source).hexdigest(), sha)
            for row in data['results']: self.assertNotIn('error', row)

    def test_sdk_removes_loops_and_rejects_complete_collapse(self):
        good = rejected = 0
        for op, row in zip(cases(True), native('inset_cusp_sdk')['results']):
            value = row['value']; self.assertTrue(value['source_unchanged'])
            if op['scene'] == 'ellipse' and op['distance'] == 4.:
                self.assertIsNone(value['output']); rejected += 1
                continue
            self.assertTrue(value['output']['valid']); good += 1
            geometry = value['output']['geometry']
            self.assertEqual(len(geometry['faces']), 2)
            clipped = op['scene'] == 'ellipse' and op['distance'] > 2.6 or op['scene'] == 'bezier' and op['distance'] == 2.8
            self.assertEqual(len(geometry['edges']), 3 if clipped else 2)
            if op['scene']=='ellipse' and op['distance']==3.99:
                exact=(10./3.)*math.sqrt(1.-((1.5*3.99)**2-16.)/20.)
                tip=geometry['vertices'][1]['point']
                error=abs(abs(tip[0])-exact)
                self.assertGreater(error,1.2e-5)
                self.assertLess(error,1.3e-5)
        self.assertEqual((good, rejected), (22, 2))

    def test_command_outputs_history_and_failed_source_retention(self):
        good = rejected = 0
        for op, row in zip(cases(), native('inset_cusp_command')['results']):
            value = row['value']
            self.assertNotIn('Unknown command:', value['history'])
            success = not (op['scene'] == 'ellipse' and op['distance'] == 4.)
            self.assertEqual(value['results'], ['Success' if success else 'Failure'])
            if not success:
                rejected += 1
                self.assertEqual(len(value['after']), 1)
                self.assertEqual(value['before'][0]['brep'], value['after'][0]['brep'])
                continue
            good += 1
            self.assertEqual(value['before'][0]['brep'], value['undo'][0]['brep'])
            self.assertEqual(value['after_script'], value['redo'])
            if not op['split']:
                self.assertEqual(value['before'][0]['brep'], value['after'][0]['brep'])
                self.assertTrue(all('definition' in o['curve'] for o in value['after'][1:]))
            elif not op['join_outputs']:
                self.assertTrue(all(o['source'] is None for o in value['after']))
        self.assertEqual((good, rejected), (66, 6))

    def test_unowned_and_malformed_recipes_decline(self):
        for op in (cases()[0], cases(True)[0]):
            validate(op)
            for changes in ({'id': 'unowned'}, {'distance': 3}, {'loose': 0}, {'crease_corners': 1},
                            {'variant': 'unknown'}, {'extra': True}):
                with self.assertRaises(ValueError): validate(dict(op, **changes))
        for op, key in ((cases()[0], 'split'), (cases()[0], 'join_outputs'), (cases(True)[0], 'ignore_seams')):
            with self.assertRaises(ValueError): validate(dict(op, **{key: 1}))


if __name__ == '__main__': unittest.main()
