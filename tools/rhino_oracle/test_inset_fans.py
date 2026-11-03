"""Immutable diagnostic evidence for Inset cusp topology and offset bases."""
import collections
import gzip
import hashlib
import json
import math
import unittest
from .test_inset_continuity import native
from .capture_family import HERE
from .generate_inset_surface import request
from . import inset_fan_basis_probe as basis
from . import inset_fan_fit_probe as fit
from . import inset_fan_interior_probe as interior
from . import inset_fan_surface_probe as surface_fit
from . import inset_fan_commands_probe as commands


FAMILIES = (
    ('inset_fan_basis', basis.cases, basis.validate),
    ('inset_fan_fit', fit.cases, fit.validate),
    ('inset_fan_surface_fit', surface_fit.cases, surface_fit.validate),
    ('inset_fan_refinement', basis.refinement, basis.validate),
    ('inset_fan_interior', interior.cases, interior.validate),
)


def topology(value):
    geometry = value['output']['geometry']
    return tuple(len(geometry[k]) for k in ('faces', 'edges', 'vertices'))


class InsetFanDiagnosticsTests(unittest.TestCase):
    def test_owned_recipes_and_immutable_producer_snapshots(self):
        total = 0
        for family, cases, validate in FAMILIES:
            operations = cases()
            query = request(); query['operations'] = operations
            path = HERE / ('fixtures/' + family + '.json')
            data = native(family)
            self.assertEqual(json.loads(path.read_bytes()), query)
            self.assertEqual(data['engine'], 'rhino')
            self.assertEqual(data['engine_version'], '8.32.26160.13001')
            self.assertEqual([r['id'] for r in data['results']], [o['id'] for o in operations])
            self.assertEqual(data['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            for sha in data['provenance']['sources'].values():
                frozen = gzip.decompress((HERE / ('observations/source_snapshots/' + sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(frozen).hexdigest(), sha)
            for op, row in zip(operations, data['results']):
                validate(op)
                self.assertNotIn('error', row)
                self.assertTrue(row['value']['source_unchanged'])
            total += len(operations)
        self.assertEqual(total, 68)

    def test_direct_and_surface_offsets_are_distinct_cubic_controls(self):
        # These public APIs do not supply the original-degree Inset basis.
        for family in ('inset_fan_fit', 'inset_fan_surface_fit'):
            for row in native(family)['results']:
                value = row['value']
                self.assertEqual(value['before']['degree'], 2)
                self.assertTrue(value['outputs'])
                for curve in value['outputs']:
                    self.assertEqual(curve['definition']['degree'], 3)
        for row in native('inset_fan_basis')['results'][6::2]:
            cap = row['value']['output']['geometry']['edges'][3]['curve']['definition']
            self.assertEqual(cap['degree'], 2)

    def test_outward_rejections_and_expanding_hole_crossings(self):
        rows = native('inset_fan_basis')['results']
        self.assertTrue(all(r['value']['output'] is None for r in rows[:6]))
        expected = [(3, 4, 4), (4, 8, 6), (3, 4, 4), (4, 8, 6), (3, 4, 4), (6, 13, 9)]
        for row, counts in zip(rows[6:], expected):
            self.assertTrue(row['value']['output']['valid'])
            self.assertEqual(topology(row['value']), counts)
        # A ray from the original hole seam (4,2) to cap corner (-4,-3.5)
        # crosses the source bottom y=-2 at x=-20/11. Splitting source edges
        # indiscriminately at intrinsic G2 breaks cannot reproduce this node.
        points = [v['point'] for v in rows[-1]['value']['output']['geometry']['vertices']]
        self.assertTrue(any(math.dist(p, [-20. / 11., -2., 0.]) < 1e-10 for p in points))
        self.assertEqual(len(rows[-1]['value']['before']['geometry']['vertices']), 2)

    def test_interior_radii_degrees_and_subdivisions_record_complete_topology(self):
        rows = native('inset_fan_interior')['results']
        expected = [(129,260,132),(67,136,70),(51,104,54),(15,32,18),
                    (87,176,90),(27,56,30),(15,32,18),(9,20,12),
                    (7,16,10),(7,16,10),(7,16,10),(129,264,136),
                    (51,108,58),(15,36,22),(7,20,14)]
        for row, counts in zip(rows, expected):
            self.assertTrue(row['value']['output']['valid'])
            self.assertEqual(topology(row['value']), counts, row['id'])
        # Cap crossings produce small cells beyond one triangle per cap edge.
        faces = rows[0]['value']['output']['geometry']['topology']['faces']
        sizes = collections.Counter(sum(len(loop['trims']) for loop in face['loops']) for face in faces)
        self.assertEqual(sizes, {3:94, 4:16, 5:16, 21:2, 48:1})

    def test_tolerance_controls_change_curvature_partitions(self):
        expected = [(9,23,16),(16,42,28),(32,84,54),(48,126,80),
                    (6,14,10),(4,8,6),(8,19,13),(18,46,30),
                    (30,77,49),(42,111,71),(26,71,47),(6,13,9),
                    (6,14,10),(8,19,13)]
        for row, counts in zip(native('inset_fan_refinement')['results'], expected):
            self.assertTrue(row['value']['output']['valid'])
            self.assertEqual(topology(row['value']), counts, row['id'])

    def test_crossed_cap_command_modes_and_immutable_provenance(self):
        family='inset_fan_commands';operations=commands.cases()
        path=HERE / ('fixtures/' + family + '.json');data=native(family)
        query=request();query['operations']=operations
        self.assertEqual(json.loads(path.read_bytes()),query)
        self.assertEqual([r['id'] for r in data['results']],[o['id'] for o in operations])
        self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
        for sha in data['provenance']['sources'].values():
            frozen=gzip.decompress((HERE / ('observations/source_snapshots/'+sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(frozen).hexdigest(),sha)
        self.assertEqual([len(r['value']['after']) for r in data['results']],[96,129,1,7,6,1])
        for op,row in zip(operations,data['results']):
            commands.validate(op);v=row['value']
            self.assertEqual(v['results'],['Success']);self.assertNotIn('Unknown command:',v['history'])
            self.assertEqual(v['before'][0]['brep'],v['undo'][0]['brep'])
            self.assertEqual(v['after_script'],v['redo'])
            for changes in ({'id':'unowned'},{'extra':True},{'split':1},{'distance':True}):
                with self.assertRaises(ValueError): commands.validate(dict(op,**changes))

    def test_malformed_recipes_decline(self):
        for _, cases, validate in FAMILIES:
            for op in cases():
                for changes in ({'id':'unowned'}, {'extra':True}, {'distance':True}, {'distance':float('nan')}):
                    with self.assertRaises(ValueError): validate(dict(op, **changes))
                for key in ('subdivide', 'degree', 'crease_corners', 'loose', 'ignore_seams'):
                    if key in op:
                        bad = int(op[key]) if type(op[key]) is bool else float(op[key])
                        with self.assertRaises(ValueError): validate(dict(op, **{key:bad}))


if __name__ == '__main__': unittest.main()
