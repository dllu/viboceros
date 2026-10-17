"""Bounded spiral recipes, isolated helper staging and public input receipts."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from unittest import TestCase
from .spiral_command_probe import validate
from .structural_sdk import stage_helpers
from .client import OracleClient, OracleProtocolError
BASE = Path(__file__).parent

class SpiralCommandTests(TestCase):

    def test_bounds_and_standalone_worker_dependencies(self):
        q = json.loads((BASE / 'fixtures/spiral_command.json').read_text())
        for op in q['operations']:
            validate(op)
        for key, value in [('id', 'bad\ninput'), ('extra', True), ('actions', []), ('actions', [True]), ('actions', [float('inf')]), ('actions', [[0, 0, float('nan')]]), ('actions', ['Cancel']), ('actions', ['Turns=0']), ('actions', ['Pitch=nan']), ('actions', ['ReverseTwist=Maybe']), ('actions', ['Vertical'] * 25), ('live_getters', 1)]:
            op = copy.deepcopy(q['operations'][0])
            op[key] = value
            with self.assertRaises(ValueError, msg=(key, value)):
                validate(op)
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            stage_helpers(q, BASE, job)
            self.assertEqual({p.name for p in job.iterdir()}, {'geometry_validity_probe.py', 'spiral_command_probe.py', 'curve_primitive_probe.py', 'join_probe.py', 'merge_edges_probe.py', 'conic_getter_input.py', 'managed_mouse_callback.py', 'snap_environment.py'})
            subprocess.run([sys.executable, '-I', '-c', 'import sys,json;sys.path.insert(0,sys.argv[1]);import spiral_command_probe;spiral_command_probe.validate(json.loads(sys.argv[2]))', str(job), json.dumps(q['operations'][0])], check=True, capture_output=True)

    def test_native_getter_receipts_curve_layout_and_real_undo_redo(self):
        for profile in ['spiral_command', 'spiral_preferences', 'spiral_defaults', 'spiral_around_curve', 'spiral_around_inputs', 'spiral_around_topology', 'spiral_relative_points']:
            q = json.loads((BASE / ('fixtures/' + profile + '.json')).read_text())
            n = json.loads((BASE / ('observations/' + profile + '_rhino_20261017.json')).read_text())
            self.assertEqual(len(q['operations']), len(n['results']))
            for op, row in zip(q['operations'], n['results']):
                self.assertEqual(op['id'], row['id'])
                v = row['value']
                command = 'Helix' if op['op'] == 'helix_command' else 'Spiral'
                self.assertTrue(v['command_returned'], op['id'])
                self.assertEqual([(e['name'], e['result']) for e in v['events'] if e['name'] == command], [(command, 'Success')])
                self.assertTrue(all((e['name'] in (command, 'Pause', 'Enter', 'SelID') and e['result'] == 'Success' for e in v['events'])))
                self.assertEqual(v['after']['outputs'], v['after_script']['outputs'])
                self.assertEqual(v['before']['outputs'], v['undo']['outputs'])
                self.assertEqual(v['after']['outputs'], v['redo']['outputs'])
                self.assertTrue(all((v[stage]['guide_unchanged'] for stage in ['before', 'after', 'undo', 'redo'])))
                self.assertTrue(v['undo_returned'])
                self.assertTrue(v['redo_returned'])
                if op.get('live_getters'):
                    trace = v['input_trace']
                    self.assertEqual(trace['errors'], [])
                    self.assertEqual(trace['completed'], len(op['actions']))
                    self.assertEqual(trace['expected'], len(op['actions']))
                    if 'setup' in op:
                        trace = v['setup_input_trace']
                        self.assertEqual(trace['errors'], [])
                        self.assertEqual(trace['completed'], len(op['setup']))
                        self.assertEqual(trace['expected'], len(op['setup']))
                self.assertEqual(len(v['after']['outputs']), 1)
                output = v['after']['outputs'][0]
                self.assertFalse(output['selected'])
                self.assertEqual(output['curve']['degree'], 3)
                for word in ['_Cancel', 'Unknown command', 'Invalid']:
                    self.assertNotIn(word, v['history'])

    def test_rail_source_bounds_and_real_edge_acknowledgements(self):
        q = json.loads((BASE / 'fixtures/spiral_around_curve.json').read_text())
        for op in q['operations']:
            validate(op)
        rail = copy.deepcopy(q['operations'][1])
        for key, value in [('degree', True), ('degree', 0), ('degree', 17), ('knots', [0, 1]), ('knots', [0, 0, 0, 1, 0, 1]), ('control_points', []), ('control_points', [{'point': [0, 0, 0], 'weight': -1}] * 3), ('control_points', [{'point': [0, 0, 10000000.0]}] * 3)]:
            op = copy.deepcopy(rail)
            op['curves'][0][key] = value
            with self.assertRaises(ValueError, msg=(key, value)):
                validate(op)
        for changes in [dict(curves=[rail['curves'][0]] * 5), dict(prior_point=[True, 0, 0]), dict(guide_selection=1), dict(actions=['AroundCurve', {'curve': True}]), dict(actions=['AroundCurve', {'relative': [0, float('inf'), 0]}]), dict(actions=['AroundCurve', {'curve': 1}]), dict(actions=['AroundCurve', 'NumPointsPerTurn', 4]), dict(actions=['AroundCurve', 'NumPointsPerTurn', 5.5]), dict(actions=['AroundCurve', 'NumPointsPerTurn', 129])]:
            with self.assertRaises(ValueError, msg=changes):
                validate(dict(rail, **changes))
        for profile in ['spiral_around_inputs', 'spiral_around_topology']:
            q = json.loads((BASE / ('fixtures/' + profile + '.json')).read_text())
            n = json.loads((BASE / ('observations/' + profile + '_rhino_20261017.json')).read_text())
            for op, row in zip(q['operations'], n['results']):
                edges = [(i, a) for i, a in enumerate(op['actions']) if isinstance(a, dict) and 'edge' in a]
                trace = row['value']['input_trace']
                self.assertEqual(len(trace['edge_picks']), len(edges))
                for (i, a), receipt in zip(edges, trace['edge_picks']):
                    self.assertEqual(receipt['action'], i)
                    self.assertEqual(receipt['authored_edge'], a['edge'])
                    self.assertEqual(receipt['pick'], a['pick'])
                    self.assertTrue(receipt['acknowledged'])
                    self.assertEqual(row['value']['after']['guide_selected'], [False])
                    with self.assertRaises(ValueError):
                        validate(dict(op, live_getters=False))
                    with self.assertRaises(ValueError):
                        validate(dict(op, setup=op['actions']))
                if not edges:
                    self.assertEqual(row['value']['after']['guide_selected'], [True])
                    self.assertEqual(row['value']['undo']['guide_selected'], [False])
                    self.assertEqual(row['value']['redo']['guide_selected'], [False])

    def test_native_flat_equal_radii_produce_no_curve(self):
        q = json.loads((BASE / 'fixtures/spiral_flat_equal_radius.json').read_text())
        n = json.loads((BASE / 'observations/spiral_flat_equal_radius_rhino_20261017.json').read_text())
        self.assertEqual(len(q['operations']), 1)
        self.assertEqual(len(n['results']), 1)
        validate(q['operations'][0])
        row = n['results'][0]
        self.assertEqual(row['id'], q['operations'][0]['id'])
        v = row['value']
        self.assertEqual(v['input_trace']['errors'], [])
        self.assertEqual(v['input_trace']['completed'], len(q['operations'][0]['actions']))
        self.assertEqual([(e['name'], e['result']) for e in v['events'] if e['name'] == 'Spiral'], [('Spiral', 'Failure')])
        for stage in ['before', 'after', 'after_script', 'undo', 'redo']:
            self.assertEqual(v[stage]['outputs'], [])

    def test_native_closed_sdk_and_command_share_periodic_control_data(self):
        command = json.loads((BASE / 'observations/spiral_around_topology_rhino_20261017.json').read_text())['results'][2]['value']['after']['outputs'][0]['curve']
        sdk = json.loads((BASE / 'observations/spiral_closed_diagnostic_rhino_20261017.json').read_text())
        self.assertEqual(sdk['results'][1]['id'], 'closed-spatial-sdk')
        self.assertEqual(sdk['results'][1]['value'], command)
        controls = command['control_points']
        self.assertEqual(controls[:3], controls[-3:])
        self.assertEqual(len(sdk['results'][0]['value']), 15)

    def test_live_jobs_need_private_bounded_dedicated_requests(self):
        import os
        from unittest import mock
        q = json.loads((BASE / 'fixtures/spiral_command.json').read_text())
        with mock.patch.dict(os.environ, dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':301'), clear=True), mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            for i, request in enumerate([q, dict(q, iterations=2), dict(q, operations=q['operations'] * 2), dict(q, operations=q['operations'] + [dict(op='line', id='foreign')])]):
                with self.assertRaises((ValueError, OracleProtocolError)):
                    OracleClient(launcher=Path('/bin/true'), settings_scheme=None if i == 0 else 'VibocerosOracleSpiralUnit').run_rhino(request, 1)
            launch.assert_not_called()

    def test_layout_probe_bounds_and_standalone_staging(self):
        from .spiral_layout_probe import validate as layout
        q = json.loads((BASE / 'fixtures/spiral_layout.json').read_text())
        for op in q['operations']:
            layout(op)
        for t in [True, 0, 0.0001, 1001, float('nan'), float('inf')]:
            with self.assertRaises(ValueError):
                layout(dict(op='spiral_layout', id='bounds', turns=t))
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            stage_helpers(q, BASE, job)
            self.assertEqual({p.name for p in job.iterdir()}, {'spiral_layout_probe.py'})
            subprocess.run([sys.executable, '-I', '-c', 'import sys,json;sys.path.insert(0,sys.argv[1]);import spiral_layout_probe;spiral_layout_probe.validate(json.loads(sys.argv[2]))', str(job), json.dumps(q['operations'][0])], check=True, capture_output=True)
