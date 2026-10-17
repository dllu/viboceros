"""Parabola authored input bounds, isolated staging and actual native receipts."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from unittest import TestCase
from .parabola_probe import validate
from .structural_sdk import stage_helpers

BASE = Path(__file__).parent


class ParabolaTests(TestCase):
    def test_bounds_and_standalone_worker_dependencies(self):
        q = json.loads((BASE / 'fixtures/parabola_command.json').read_text())
        for op in q['operations']:
            validate(op)
        for key, value in [('id', 'bad\ninput'), ('command', 'Line'), ('actions', []),
                           ('actions', ['Focus'] * 21), ('actions', [True]),
                           ('actions', [[0, 0, float('nan')]]), ('actions', [float('inf')]),
                           ('actions', ['Cancel']), ('actions', ['MarkFocus=Maybe']),
                           ('setup', ['Unknown']), ('construction_plane', [[0, 0, 0]]),
                           ('extra', 'unexpected')]:
            op = copy.deepcopy(q['operations'][0])
            op[key] = value
            with self.assertRaises(ValueError, msg=(key, value)):
                validate(op)
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            stage_helpers(q, BASE, job)
            self.assertEqual({p.name for p in job.iterdir()},
                             {'parabola_probe.py', 'curve_primitive_probe.py', 'join_probe.py', 'merge_edges_probe.py'})
            subprocess.run([sys.executable, '-I', '-c',
                            'import sys,json;sys.path.insert(0,sys.argv[1]);import parabola_probe,curve_primitive_probe;'
                            'parabola_probe.validate(json.loads(sys.argv[2]))',
                            str(job), json.dumps(q['operations'][0])], check=True, capture_output=True)

    def test_native_modes_orders_focus_object_order_and_real_undo_redo(self):
        q = json.loads((BASE / 'fixtures/parabola_command.json').read_text())
        n = json.loads((BASE / 'observations/parabola_command_rhino_20261017.json').read_text())
        self.assertEqual(len(q['operations']), 24)
        self.assertEqual(len(n['results']), 24)
        for op, row in zip(q['operations'], n['results']):
            self.assertEqual(op['id'], row['id'])
            v = row['value']
            self.assertTrue(v['command_returned'])
            self.assertEqual([(e['name'], e['result']) for e in v['events']], [(op['command'], 'Success')])
            self.assertEqual(v['after'], v['after_script'])
            self.assertTrue(v['undo_returned'])
            self.assertTrue(v['redo_returned'])
            self.assertEqual(v['before'], v['undo'])
            self.assertEqual(v['after'], v['redo'])
            for token in ['_Cancel', 'Unknown command', 'Invalid']:
                self.assertNotIn(token, v['history'])
            outputs = v['after']['outputs']
            self.assertIn(len(outputs), (1, 2))
            if len(outputs) == 2:
                self.assertIn('point', outputs[0])
            self.assertEqual(outputs[-1]['curve']['degree'], 2)
            self.assertEqual(outputs[-1]['curve']['domain'], [0., 1.])
            self.assertTrue(all(not o['selected'] for o in outputs))

    def test_native_wrong_direction_anchor_controls_keep_success_without_output(self):
        q = json.loads((BASE / 'fixtures/parabola_direction_controls.json').read_text())
        n = json.loads((BASE / 'observations/parabola_direction_controls_rhino_20261017.json').read_text())
        self.assertEqual(len(q['operations']), 3)
        for op, row in zip(q['operations'], n['results']):
            validate(op)
            self.assertEqual(op['id'], row['id'])
            v = row['value']
            self.assertTrue(v['command_returned'])
            self.assertEqual([(e['name'], e['result']) for e in v['events']], [('Parabola3Pt', 'Success')])
            self.assertEqual(v['after']['outputs'], [])
