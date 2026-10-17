"""Clamped Circle layouts, owned getters and explicit-accuracy length witnesses."""
import json
from pathlib import Path
from unittest import TestCase
from .circle_around_probe import validate

BASE = Path(__file__).parent


class CircleClampedTests(TestCase):
    def profile(self, name):
        q = json.loads((BASE / ('fixtures/circle_clamped_' + name + '.json')).read_text())
        n = json.loads((BASE / ('observations/circle_clamped_' + name + '_rhino_20261017.json')).read_text())
        self.assertEqual(len(q['operations']), len(n['results']))
        return list(zip(q['operations'], n['results']))

    def test_all_clamped_layouts_modes_receipts_and_history(self):
        layouts = set()
        total = edges = 0
        for name, count in [('grid', 43), ('modes', 14), ('precise', 2)]:
            rows = self.profile(name)
            self.assertEqual(len(rows), count)
            for op, row in rows:
                validate(op)
                self.assertEqual(op['id'], row['id'])
                v = row['value']
                t = v['input_trace']
                self.assertEqual(t['errors'], [])
                self.assertEqual(t['completed'], len(op['actions']))
                self.assertNotIn('Unknown command', v['history'])
                self.assertEqual([e['result'] for e in v['events'] if e['name'] == 'Circle'], ['Success'])
                self.assertTrue(v['undo_returned'] and v['redo_returned'])
                self.assertEqual(v['before']['outputs'], v['undo']['outputs'])
                self.assertEqual(v['after_script']['outputs'], v['redo']['outputs'])
                for stage in ['before', 'after', 'after_script', 'undo', 'redo']:
                    self.assertTrue(v[stage]['guide_unchanged'])
                c = v['after_script']['outputs'][0]['curve']
                degree = c['degree']
                self.assertTrue(all(p['weight'] == 1.0 for p in c['control_points']))
                self.assertNotEqual(c['control_points'][:degree], c['control_points'][-degree:])
                self.assertEqual(c['knots'][:degree+1], [0.] * (degree+1))
                self.assertEqual(c['knots'][-degree-1:], [c['domain'][1]] * (degree+1))
                if name in ('grid', 'precise'):
                    count = int(next(a.split('=')[1] for a in op['actions'] if isinstance(a, str) and a.startswith('PointCount=')))
                    layouts.add((degree, count))
                    self.assertEqual(len(c['control_points']), max(count, degree) + 1)
                if 'setup' in op:
                    self.assertEqual(v['setup_input_trace']['errors'], [])
                    self.assertEqual(v['setup_input_trace']['completed'], len(op['setup']))
                for receipt in t['edge_picks']:
                    self.assertTrue(receipt['acknowledged'])
                    edges += 1
                total += 1
        minimum = [4, 4, 5, 6, 7, 9, 10, 12, 13, 15]
        self.assertEqual(layouts, {(d, n) for d in range(2, 12) for n in range(4, minimum[d-2])})
        self.assertEqual((total, edges), (59, 1))
        for op, row in self.profile('controls'):
            validate(op)
            v = row['value']
            self.assertEqual(v['input_trace']['errors'], [])
            self.assertEqual(v['input_trace']['completed'], len(op['actions']))
            self.assertEqual([e['result'] for e in v['events'] if e['name'] == 'Circle'], ['Cancel'])
            for stage in ['before', 'after', 'after_script', 'undo', 'redo']:
                self.assertEqual(v[stage]['outputs'], [])

    def test_precise_recap_preserves_geometry_and_changes_only_length_metric(self):
        default = {op['id']: row for op, row in self.profile('length_diagnostics')}
        for op, row in self.profile('precise'):
            self.assertTrue(op['precise_length'])
            original = default[op['id'].removesuffix('-precise')]
            for stage in ['after', 'after_script', 'redo']:
                a = row['value'][stage]['outputs'][0]
                b = original['value'][stage]['outputs'][0]
                self.assertEqual({k:v for k,v in a.items() if k != 'length'}, {k:v for k,v in b.items() if k != 'length'})
                self.assertGreater(abs(a['length'] - b['length']), 1e-6)

    def test_accuracy_flag_remains_bounded_to_circle_deform(self):
        op = self.profile('precise')[0][0]
        validate(op)
        for value in [1, 'yes', None, {}, 1e-12]:
            with self.assertRaises(ValueError):
                validate(dict(op, precise_length=value))
        with self.assertRaises(ValueError):
            validate(dict(op='circle_around', id='circle-around-length-flag', live_getters=True, actions=['AroundCurve'], precise_length=True))
