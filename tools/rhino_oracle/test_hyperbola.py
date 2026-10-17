"""Bounded Hyperbola construction, coefficient getters and native command witnesses."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from unittest import TestCase
from .hyperbola_probe import validate
from .structural_sdk import stage_helpers

BASE = Path(__file__).parent


class HyperbolaTests(TestCase):
    def test_bounded_inputs_and_standalone_staged_dependencies(self):
        request = json.loads((BASE / 'fixtures/hyperbola_command.json').read_text())
        for op in request['operations']:
            validate(op)
        for key, value in [('id', 'bad\ninput'), ('actions', []), ('actions', ['FromFoci'] * 25),
                           ('actions', [True]), ('actions', [[0, 0, float('inf')]]),
                           ('actions', ['A=0']), ('actions', ['B=-1']), ('actions', ['A=nan']),
                           ('actions', ['A=1\n_Cancel']), ('actions', ['Unknown']),
                           ('actions', ['Cancel']), ('actions', [float('nan')]),
                           ('setup', ['Unknown']), ('live_getters', 1),
                           ('construction_plane', [[0, 0, 0]]), ('extra', 0)]:
            op = copy.deepcopy(request['operations'][0])
            op[key] = value
            with self.assertRaises(ValueError, msg=(key, value)):
                validate(op)
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            stage_helpers(request, BASE, job)
            self.assertEqual({p.name for p in job.iterdir()}, {'hyperbola_probe.py', 'curve_primitive_probe.py',
                             'join_probe.py', 'merge_edges_probe.py', 'conic_getter_input.py',
                             'managed_mouse_callback.py', 'snap_environment.py'})
            subprocess.run([sys.executable, '-I', '-c',
                            'import sys,json;sys.path.insert(0,sys.argv[1]);import hyperbola_probe,curve_primitive_probe;'
                            'hyperbola_probe.validate(json.loads(sys.argv[2]))', str(job),
                            json.dumps(request['operations'][0])], check=True, capture_output=True)
        from .conic_getter_input import drive
        with self.assertRaises(ValueError):
            drive({}, {}, [], command='Unsupported')

    def test_native_geometry_objects_selection_and_actual_undo_redo(self):
        request = json.loads((BASE / 'fixtures/hyperbola_command.json').read_text())
        native = json.loads((BASE / 'observations/hyperbola_command_rhino_20261017.json').read_text())
        self.assertEqual(len(request['operations']), 25)
        self.assertEqual(len(native['results']), 25)
        for op, row in zip(request['operations'], native['results']):
            self.assertEqual(op['id'], row['id'])
            v = row['value']
            self.assertTrue(v['command_returned'])
            self.assertEqual([(e['name'], e['result']) for e in v['events']], [('Hyperbola', 'Success')])
            self.assertEqual(v['after'], v['after_script'])
            self.assertTrue(v['undo_returned'])
            self.assertTrue(v['redo_returned'])
            self.assertEqual(v['before'], v['undo'])
            self.assertEqual(v['after'], v['redo'])
            for token in ['_Cancel', 'Unknown command', 'Invalid']:
                self.assertNotIn(token, v['history'])
            outputs = v['after']['outputs']
            self.assertIn(len(outputs), (1, 2, 4))
            curves = [o for o in outputs if 'curve' in o]
            points = [o for o in outputs if 'point' in o]
            self.assertIn(len(curves), (1, 2))
            self.assertIn(len(points), (0, len(curves)))
            self.assertEqual(outputs[:len(points)], points)
            for curve in curves:
                self.assertEqual(curve['curve']['degree'], 2)
                self.assertEqual(curve['curve']['domain'], [0., 1.])
                self.assertFalse(curve['closed'])
            self.assertTrue(all(not o['selected'] for o in outputs))

    def test_all_native_profiles_keep_status_history_and_pole_query_failures_explicit(self):
        cases = {}
        for group in ('command', 'planes', 'directions', 'gates', 'boundaries', 'mode_cancel', 'projective', 'shared_driver'):
            request = json.loads((BASE / ('fixtures/hyperbola_' + group + '.json')).read_text())
            native = json.loads((BASE / ('observations/hyperbola_' + group + '_rhino_20261017.json')).read_text())
            self.assertEqual(len(request['operations']), len(native['results']))
            for op, row in zip(request['operations'], native['results']):
                validate(op)
                self.assertEqual(op['id'], row['id'])
                v = row['value']
                cases[op['id']] = v
                expected = 'Success' if v['command_returned'] else 'Cancel'
                self.assertEqual([e['result'] for e in v['events'] if e['name'] == 'Hyperbola'], [expected])
                self.assertEqual(v['after'], v['after_script'])
                self.assertEqual(v['before'], v['undo'])
                self.assertEqual(v['after'], v['redo'])
                for token in ('_Cancel', 'Unknown command', 'Invalid'):
                    self.assertNotIn(token, v['history'])
                if not v['command_returned']:
                    self.assertEqual(v['after']['outputs'], [])
                if 'input_trace' in v:
                    trace = v['input_trace']
                    self.assertEqual(trace['errors'], [])
                    self.assertEqual(trace['completed'], trace['expected'])
                for output in v['after']['outputs']:
                    if output.get('length_query_failed'):
                        self.assertEqual(output['length'], 0.)
                        self.assertLessEqual(output['curve']['control_points'][1]['weight'], -1.)
        self.assertEqual(len(cases), 86)
        self.assertEqual(sum(v['command_returned'] for v in cases.values()), 63)
        self.assertEqual(sum(not v['command_returned'] for v in cases.values()), 23)
        self.assertEqual(sum(o.get('length_query_failed', False) for v in cases.values() for o in v['after']['outputs']), 14)

    def test_shared_conic_driver_keeps_owned_mouse_and_edge_acknowledgements(self):
        native = json.loads((BASE / 'observations/conic_shared_driver_rhino_20261017.json').read_text())
        self.assertEqual(len(native['results']), 2)
        motions, edges = 0, 0
        for row in native['results']:
            v = row['value']
            self.assertTrue(v['command_returned'])
            trace = v['input_trace']
            self.assertEqual(trace['errors'], [])
            self.assertEqual(trace['completed'], trace['expected'])
            self.assertEqual(v['before'], v['undo'])
            self.assertEqual(v['after'], v['redo'])
            motions += len(trace['motion'])
            edges += len(trace['edge_picks'])
            self.assertTrue(all(m['acknowledged'] for m in trace['motion']))
            self.assertTrue(all(e['acknowledged'] for e in trace['edge_picks']))
        self.assertGreater(motions, 0)
        self.assertGreater(edges, 0)

    def test_managed_hyperbola_jobs_require_private_dedicated_bounded_requests(self):
        import os
        from unittest import mock
        from .client import OracleClient, OracleProtocolError
        request = json.loads((BASE / 'fixtures/hyperbola_shared_driver.json').read_text())
        with mock.patch.dict(os.environ, dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':301'), clear=True), mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            for index, q in enumerate([request, dict(request, iterations=2),
                                       dict(request, operations=request['operations'] * 2),
                                       dict(request, operations=request['operations'] + [dict(op='line', id='foreign')])]):
                with self.assertRaises((ValueError, OracleProtocolError)):
                    OracleClient(launcher=Path('/bin/true'), settings_scheme=None if index == 0 else 'VibocerosOracleHyperbolaUnit').run_rhino(q, 1)
            launch.assert_not_called()
