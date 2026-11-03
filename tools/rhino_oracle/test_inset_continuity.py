"""Owned G2 thresholds, native cusp fans and connector controls."""
import functools
import gzip
import hashlib
import json
import math
import unittest
from .capture_family import HERE
from .generate_inset_surface import request
from .inset_continuity_probe import cases, limits, validate
from .inset_collinear_probe import cases as collinear_cases, validate as collinear_validate


@functools.lru_cache(None)
def native(family):
    return json.loads(gzip.decompress((HERE / ('observations/' + family + '.json.gz')).read_bytes()))


class InsetContinuityTests(unittest.TestCase):
    def test_owned_recipes_and_immutable_provenance(self):
        for family, operations in (
            ('inset_continuity_sdk', cases(True)),
            ('inset_continuity_command', cases()),
            ('inset_continuity_limits', limits()),
            ('inset_collinear_sdk', collinear_cases()),
        ):
            query = request(); query['operations'] = operations
            path = HERE / ('fixtures/' + family + '.json'); data = native(family)
            self.assertEqual(json.loads(path.read_bytes()), query)
            self.assertEqual([r['id'] for r in data['results']], [o['id'] for o in operations])
            self.assertEqual(data['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            for sha in data['provenance']['sources'].values():
                source = gzip.decompress((HERE / ('observations/source_snapshots/' + sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(source).hexdigest(), sha)
            for row in data['results']: self.assertNotIn('error', row)

    def test_regular_groups_and_explicit_pending_native_cusp_fans(self):
        good = rejected = fans = 0
        for op, row in zip(cases(True), native('inset_continuity_sdk')['results']):
            v = row['value']; self.assertTrue(v['source_unchanged'])
            if v['output'] is None:
                rejected += 1; continue
            good += 1; self.assertTrue(v['output']['valid']); g = v['output']['geometry']
            if op['scene'] == 'ellipse':
                # SplitEdge already made independent entries; reassignment is
                # a negative control, rather than evidence for shared storage.
                self.assertEqual(v['source_curve_indices'], [0, 1, 2, 3])
                expected = (2, 8, 8) if op['loose'] else (2, 5, 5)
            elif op['scene'] == 'asymmetric':
                if not op['crease_corners']:
                    expected = (2, 3, 3) if op['distance'] == 0.3 else (2, 4, 4)
                else:
                    expected = {0.3: (3, 6, 4), 2.: (79, 158, 80), 3.: (7, 14, 8)}[op['distance']]
                    fans += int(op['distance'] != 0.3)
            else:
                split = op['variant'] == 'split'
                if not op['crease_corners']:
                    cap = 1 if op['distance'] == 0.3 else 3
                    n = 8 if split else 4; expected = (2, n + cap, n + cap)
                elif op['distance'] == 0.3:
                    expected = (7, 18, 12) if split else (5, 12, 8)
                else:
                    expected = (67, 140, 74) if split else (67, 136, 70); fans += 1
            self.assertEqual(tuple(len(g[k]) for k in ('faces', 'edges', 'vertices')), expected, op['id'])
        self.assertEqual((good, rejected, fans), (22, 6, 4))

    def test_small_curvature_jumps_use_the_native_g2_relative_tolerance(self):
        for op, row in zip(limits(), native('inset_continuity_limits')['results']):
            v = row['value']; self.assertTrue(v['source_unchanged']); self.assertTrue(v['output']['valid'])
            curvatures = v['source_endpoint_curvatures']
            self.assertGreater(math.dist(curvatures[0][1], curvatures[1][0]), 1e-5)
            g = v['output']['geometry']
            expected = (3, 6, 4) if op['lower_radius'] == 3.5 and op['crease_corners'] else (2, 3, 3)
            self.assertEqual(tuple(len(g[k]) for k in ('faces', 'edges', 'vertices')), expected)

    def test_commands_keep_sources_history_and_native_fan_piece_counts(self):
        good = rejected = 0
        for op, row in zip(cases(), native('inset_continuity_command')['results']):
            v = row['value']; self.assertNotIn('Unknown command:', v['history'])
            if v['results'] == ['Failure']:
                rejected += 1; self.assertEqual(len(v['after']), 1)
                self.assertEqual(v['before'][0]['brep'], v['after'][0]['brep']); continue
            good += 1; self.assertEqual(v['results'], ['Success'])
            self.assertEqual(v['before'][0]['brep'], v['undo'][0]['brep'])
            self.assertEqual(v['after_script'], v['redo'])
            sdk_id = op['id'].rsplit('_', 1)[0] + '_sdk'
            sdk = next(r['value'] for r in native('inset_continuity_sdk')['results'] if r['id'] == sdk_id)
            if not op['split']:
                n = len(sdk['before']['geometry']['vertices'])
                connectors = sum(min(e) < n <= max(e) for e in sdk['output']['geometry']['topology']['edges'])
                self.assertEqual(len(v['after']), 2 + connectors)
                self.assertEqual(v['before'][0]['brep'], v['after'][0]['brep'])
            elif op['join_outputs']:
                self.assertEqual(len(v['after']), 1)
            else:
                self.assertEqual(len(v['after']), len(sdk['output']['geometry']['faces']))
        self.assertEqual((good, rejected), (66, 18))

    def test_off_center_connectors_choose_nearer_endpoints_and_record_midpoints(self):
        for op, row in zip(collinear_cases(), native('inset_collinear_sdk')['results']):
            v = row['value']; self.assertTrue(v['source_unchanged']); self.assertTrue(v['output']['valid'])
            g = v['output']['geometry']; points = [p['point'] for p in g['vertices']]
            self.assertEqual(tuple(len(g[k]) for k in ('faces', 'edges', 'vertices')), (6, 14, 9))
            edge = next(e for e in g['topology']['edges'] if 4 in e and max(e) >= 5)
            endpoint = points[max(edge)]
            if op['fraction'] != 0.5:
                self.assertEqual(endpoint, min(points[5:], key=lambda p: math.dist(p, points[4])))
            else:
                self.assertLess(endpoint[0], 0.)

    def test_malformed_recipes_decline(self):
        for op in cases()[:1] + cases(True)[:1] + limits()[:1]:
            validate(op)
            for changes in ({'id': 'unowned'}, {'loose': 0}, {'crease_corners': 1}, {'extra': True}):
                with self.assertRaises(ValueError): validate(dict(op, **changes))
        op = collinear_cases()[0]; collinear_validate(op)
        for changes in ({'fraction': 0}, {'fraction': 0.2}, {'loose': 0}, {'ignore_seams': 1}, {'extra': True}):
            with self.assertRaises(ValueError): collinear_validate(dict(op, **changes))


if __name__ == '__main__': unittest.main()
