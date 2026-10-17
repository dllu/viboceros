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
            self.assertEqual({p.name for p in job.iterdir()}, {'spiral_command_probe.py', 'curve_primitive_probe.py', 'join_probe.py', 'merge_edges_probe.py', 'conic_getter_input.py', 'managed_mouse_callback.py', 'snap_environment.py'})
            subprocess.run([sys.executable, '-I', '-c', 'import sys,json;sys.path.insert(0,sys.argv[1]);import spiral_command_probe;spiral_command_probe.validate(json.loads(sys.argv[2]))', str(job), json.dumps(q['operations'][0])], check=True, capture_output=True)

    def test_native_getter_receipts_curve_layout_and_real_undo_redo(self):
        for profile in ['spiral_command', 'spiral_preferences', 'spiral_defaults']:
            q = json.loads((BASE / ('fixtures/' + profile + '.json')).read_text())
            n = json.loads((BASE / ('observations/' + profile + '_rhino_20261017.json')).read_text())
            self.assertEqual(len(q['operations']), len(n['results']))
            for op, row in zip(q['operations'], n['results']):
                self.assertEqual(op['id'], row['id'])
                v = row['value']
                command = 'Helix' if op['op'] == 'helix_command' else 'Spiral'
                self.assertTrue(v['command_returned'], op['id'])
                self.assertEqual([(e['name'], e['result']) for e in v['events'] if e['name'] == command], [(command, 'Success')])
                self.assertTrue(all((e['name'] in (command, 'Pause', 'Enter') and e['result'] == 'Success' for e in v['events'])))
                self.assertEqual(v['after'], v['after_script'])
                self.assertEqual(v['before'], v['undo'])
                self.assertEqual(v['after'], v['redo'])
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
