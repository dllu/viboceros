"""Completed native TargetSurface commands and strict capture validation."""
import copy
import json
from pathlib import Path
from unittest import TestCase

from .flow_line_comparison import comparison
from .flow_line_probe import validate
from .flow_roadlike_probe import validate as roadlike
from .flow_probe import validate as sdk

BASE = Path(__file__).parent


class FlowTargetSurfaceTests(TestCase):
    def read(self, path):
        return json.loads((BASE / path).read_text())

    def data(self):
        return [self.read(p) for p in [
            'fixtures/flow_target_surface.json',
            'observations/flow_target_surface_viboceros_20261017.json',
            'observations/flow_target_surface_rhino_20261017.json']]

    def test_complete_commands_match_geometry_references_grips_groups_and_undo_redo(self):
        q, a, n = self.data()
        self.assertEqual(len(q['operations']), 98)
        for op, row in zip(q['operations'], n['results']):
            (roadlike if op['op'] == 'flow_roadlike' else validate)(op)
            self.assertEqual(op['id'], row['id'])
            v = row['value']
            for key in ['command_returned', 'undo_command_returned', 'redo_command_returned']:
                self.assertIs(v[key], True)
            self.assertNotIn('_Cancel', v['macro'])
            self.assertNotIn('Unknown command', v['history'])
            self.assertNotIn('Invalid', v['history'])
            self.assertEqual([e['result'] for e in v['events'] if e['name'] == 'Flow'], ['Success'])
        report = comparison(q, a, n)
        self.assertEqual(report['matched_cases'], 98)
        self.assertEqual(report, self.read('observations/flow_target_surface_comparison_20261017.json'))

    def test_surface_geometry_and_selection_corruption_cannot_pass(self):
        q, a, n = self.data()
        for field, value in [('guide_unchanged', False), ('guide_selected', True)]:
            damaged = copy.deepcopy(a)
            damaged['results'][0]['value']['after'][field] = value
            self.assertEqual(comparison(q, damaged, n)['matched_cases'], 97)
        for value in [.01, float('nan')]:
            damaged = copy.deepcopy(a)
            damaged['results'][0]['value']['after']['outputs'][0]['points'][0][0] = value
            self.assertEqual(comparison(q, damaged, n)['matched_cases'], 97)

    def test_curved_patch_frames_and_rational_rigid_poses_match_native(self):
        name = 'flow_target_surface_patches'
        q = self.read('fixtures/' + name + '.json')
        a = self.read('observations/' + name + '_viboceros_20261017.json')
        n = self.read('observations/' + name + '_rhino_20261017.json')
        for op, row in zip(q['operations'], n['results']):
            validate(op)
            v = row['value']
            self.assertEqual([e['result'] for e in v['events'] if e['name'] == 'Flow'], ['Success'])
            self.assertNotIn('_Cancel', v['macro'])
            self.assertNotIn('Unknown command', v['history'])
        report = comparison(q, a, n)
        self.assertEqual(report['matched_cases'], 20)
        self.assertEqual(report, self.read('observations/' + name + '_comparison_20261017.json'))

    def test_unstretched_prefix_lookup_and_corner_match_public_native_queries(self):
        for name, count in [('flow_target_surface_lookup', 2), ('flow_target_surface_corner', 1)]:
            q = self.read('fixtures/' + name + '.json')
            a = self.read('observations/' + name + '_viboceros_20261017.json')
            n = self.read('observations/' + name + '_rhino_20261017.json')
            for op in q['operations']:
                (sdk if op['op'] == 'flow' else validate)(op)
            report = comparison(q, a, n)
            self.assertEqual(report['matched_cases'], count)
            self.assertEqual(report, self.read('observations/' + name + '_comparison_20261017.json'))
            if count != 2:
                continue
            op = q['operations'][0]
            value = n['results'][0]['value']
            trim = value['diagnostics']['target']['flow_trim']
            controls = op['target']['control_points']
            # Public loose inverse -> trimmed curve -> public 841-point batch
            # predicts the command's unstretched centers. A full-target table
            # and the default LengthParameter cut fail this witness.
            self.assertGreater(abs(trim['cut'] - trim['default_cut']), 1e-8)
            for i, expected in enumerate(value['sdk_points'][1:12]):
                t = trim['parameters'][84 * i]
                basis = [(1-t)**3, 3*t*(1-t)**2, 3*t*t*(1-t), t**3]
                weight = sum(c['weight'] * b for c, b in zip(controls, basis))
                actual = [sum(c['point'][j] * c['weight'] * b for c, b in zip(controls, basis)) / weight for j in range(3)]
                self.assertLess(max(abs(x-y) for x, y in zip(actual, expected)), 1e-10)

    def test_target_surface_recipe_checks_type_finite_values_and_boolean_orientation(self):
        op = self.read('fixtures/flow_target_surface.json')['operations'][0]
        nonfinite = copy.deepcopy(op['target_surface'])
        nonfinite['control_points'][0]['point'][0] = float('nan')
        # Net layout validation is delegated to the shared host constructors.
        for value in [None, {}, op['base'], nonfinite]:
            damaged = copy.deepcopy(op)
            damaged['target_surface'] = value
            with self.assertRaises(ValueError):
                validate(damaged)
        for value in [None, 1, 'Yes']:
            damaged = copy.deepcopy(op)
            damaged['target_surface_reversed'] = value
            with self.assertRaises(ValueError):
                validate(damaged)
        damaged = copy.deepcopy(op)
        del damaged['target_surface']
        damaged['target_surface_reversed'] = True
        with self.assertRaises(ValueError):
            validate(damaged)
