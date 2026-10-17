"""Bounded Catenary capture inputs, exact history receipts and native limits."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from unittest import TestCase, mock
from .catenary_command_probe import validate
from .client import OracleClient, OracleProtocolError
from .structural_sdk import stage_helpers
BASE = Path(__file__).parent


class CatenaryCommandTests(TestCase):
    def test_native_getter_geometry_markers_defaults_and_real_history(self):
        total = 0
        for profile, count in [('command', 9), ('preferences', 8), ('directions', 11), ('final', 8)]:
            q = json.loads((BASE / ('fixtures/catenary_' + profile + '.json')).read_text())
            n = json.loads((BASE / ('observations/catenary_' + profile + '_rhino_20261017.json')).read_text())
            self.assertEqual(len(q['operations']), count)
            self.assertEqual(len(n['results']), count)
            for op, row in zip(q['operations'], n['results']):
                validate(op)
                self.assertEqual(row['id'], op['id'])
                v = row['value']
                self.assertTrue(v['command_returned'])
                self.assertEqual([(e['name'], e['result']) for e in v['events'] if e['name'] == 'Catenary'], [('Catenary', 'Success')])
                self.assertTrue(all(e['name'] in ('Catenary', 'Pause', 'Enter') and e['result'] == 'Success' for e in v['events']))
                self.assertEqual(v['before'], v['undo'])
                self.assertEqual(v['after'], v['after_script'])
                self.assertEqual(v['after'], v['redo'])
                self.assertTrue(v['undo_returned'])
                self.assertTrue(v['redo_returned'])
                self.assertTrue(all(v[s]['guide_unchanged'] for s in ('before', 'after', 'undo', 'redo')))
                for key, actions in [('input_trace', op['actions'])] + ([('setup_input_trace', op['setup'])] if 'setup' in op else []):
                    t = v[key]
                    self.assertEqual(t['errors'], [])
                    self.assertEqual(t['completed'], len(actions))
                    self.assertEqual(t['expected'], len(actions))
                    self.assertTrue(all(t['prompts']))
                outputs = v['after']['outputs']
                curves = [o for o in outputs if 'curve' in o]
                points = [o for o in outputs if 'point' in o]
                self.assertEqual(len(curves), 1)
                self.assertIn(len(points), (0, 1))
                self.assertTrue(all(not o['selected'] for o in outputs))
                if points:
                    self.assertIn('point', outputs[0])
                self.assertIn(curves[0]['curve']['degree'], (1, 3))
                if op['id'] == 'catenary-initial-defaults':
                    self.assertEqual(len(outputs), 1)
                    self.assertEqual(curves[0]['curve']['degree'], 3)
                    self.assertEqual(len(curves[0]['curve']['control_points']), 20)
                for word in ('Unknown command', 'Invalid', '_Cancel'):
                    self.assertNotIn(word, v['history'])
                total += 1
        self.assertEqual(total, 36)

    def test_native_cancellation_failures_and_minimum_count_receipts(self):
        q = json.loads((BASE / 'fixtures/catenary_rejections.json').read_text())
        n = json.loads((BASE / 'observations/catenary_rejections_rhino_20261017.json').read_text())
        self.assertEqual(len(q['operations']), 7)
        self.assertEqual(len(n['results']), 7)
        for op, row in zip(q['operations'], n['results']):
            validate(op)
            self.assertEqual(op['id'], row['id'])
            v = row['value']
            self.assertEqual(v['input_trace']['errors'], [])
            self.assertEqual(v['input_trace']['completed'], len(op['actions']))
            results = [e['result'] for e in v['events'] if e['name'] == 'Catenary']
            self.assertIn(results, (['Cancel'], ['Failure']))
            for stage in ('before', 'after', 'after_script', 'undo', 'redo'):
                self.assertEqual(v[stage]['outputs'], [])
        q = json.loads((BASE / 'fixtures/catenary_count_limits.json').read_text())
        n = json.loads((BASE / 'observations/catenary_count_limits_rhino_20261017.json').read_text())
        self.assertEqual(len(q['operations']), 2)
        for op, row in zip(q['operations'], n['results']):
            validate(op)
            self.assertIn('PointCount=2', op['actions'])
            v = row['value']
            self.assertEqual(v['input_trace']['errors'], [])
            self.assertIn('PointCount=4', v['input_trace']['prompts'][-1])
            curve = [o['curve'] for o in v['after']['outputs'] if 'curve' in o][0]
            self.assertEqual(len(curve['control_points']), 4)

    def test_recipe_bounds_and_standalone_staging(self):
        q = json.loads((BASE / 'fixtures/catenary_command.json').read_text())
        for key, value in [('id', 'bad\ncommand'), ('extra', True), ('actions', []), ('actions', [True]), ('actions', [0]), ('actions', [float('nan')]), ('actions', [[0, 0, float('inf')]]), ('actions', ['_Delete']), ('actions', ['MarkApex=Maybe']), ('actions', ['PointCount=129']), ('actions', ['PointCount', 3.5]), ('actions', ['PointCount', 129]), ('actions', [{'hover': [0, 0, 0]}]), ('actions', [{'relative': [True, 0, 0]}]), ('actions', ['Enter'] * 33), ('live_getters', 1), ('prior_point', [0, 0, 1e7]), ('construction_plane', [[0, 0, 0]] * 2)]:
            op = dict(q['operations'][0], **{key: value})
            with self.assertRaises(ValueError, msg=(key, value)):
                validate(op)
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            stage_helpers(q, BASE, job)
            self.assertEqual({p.name for p in job.iterdir()}, {'catenary_command_probe.py', 'curve_primitive_probe.py', 'join_probe.py', 'merge_edges_probe.py', 'conic_getter_input.py', 'managed_mouse_callback.py', 'snap_environment.py'})
            subprocess.run([sys.executable, '-I', '-c', 'import sys,json;sys.path.insert(0,sys.argv[1]);import catenary_command_probe;catenary_command_probe.validate(json.loads(sys.argv[2]))', str(job), json.dumps(q['operations'][0])], check=True, capture_output=True)

    def test_live_capture_requires_private_bounded_dedicated_request(self):
        import os
        q = json.loads((BASE / 'fixtures/catenary_command.json').read_text())
        with mock.patch.dict(os.environ, dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':301'), clear=True), mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            for i, request in enumerate([q, dict(q, iterations=2), dict(q, iterations=True), dict(q, operations=q['operations'] * 2), dict(q, operations=q['operations'] + [dict(op='line', id='foreign')])]):
                with self.assertRaises((ValueError, OracleProtocolError)):
                    OracleClient(launcher=Path('/bin/true'), settings_scheme=None if i == 0 else 'VibocerosOracleCatenaryUnit').run_rhino(request, 1)
            launch.assert_not_called()

    def test_minimum_count_public_sdk_data_matches_the_command(self):
        low = json.loads((BASE / 'observations/catenary_low_count_rhino_20261017.json').read_text())
        native = json.loads((BASE / 'observations/catenary_preferences_rhino_20261017.json').read_text())
        command = next(r for r in native['results'] if r['id'] == 'catenary-smooth-four')['value']['after']['outputs'][0]['curve']
        for key in ('degree', 'control_points', 'knots', 'domain'):
            self.assertEqual(low['results'][0]['value'][key], command[key])
        for profile, count in [('low_count', 6), ('tangent_confirm', 4), ('negative_tangents', 2)]:
            q = json.loads((BASE / ('fixtures/catenary_' + profile + '.json')).read_text())
            n = json.loads((BASE / ('observations/catenary_' + profile + '_rhino_20261017.json')).read_text())
            self.assertEqual(len(q['operations']), count)
            self.assertEqual(len(n['results']), count)
            for op, row in zip(q['operations'], n['results']):
                self.assertEqual(op['id'], row['id'])
                curve = row['value'].get('curve', row['value'])
                self.assertEqual(curve['degree'], 3)
                self.assertEqual(len(curve['control_points']), 4)
