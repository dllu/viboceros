"""Bounded variable-degree Circle getter captures and full history witnesses."""
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from unittest import TestCase, mock
from .circle_around_probe import validate
from .client import OracleClient, OracleProtocolError
from .structural_sdk import stage_helpers

BASE = Path(__file__).parent


class CircleDeformTests(TestCase):
    def test_native_modes_preferences_edges_cancellation_and_excluded_attempts(self):
        total = edges = 0
        for profile, count in [('default', 1), ('modes', 22), ('extended', 7), ('boundaries', 17)]:
            q = json.loads((BASE / ('fixtures/circle_deform_' + profile + '.json')).read_text())
            n = json.loads((BASE / ('observations/circle_deform_' + profile + '_rhino_20261017.json')).read_text())
            self.assertEqual(len(q['operations']), count)
            self.assertEqual(len(n['results']), count)
            for op, row in zip(q['operations'], n['results']):
                validate(op)
                self.assertEqual(op['id'], row['id'])
                v = row['value']
                self.assertEqual(v['input_trace']['errors'], [])
                self.assertEqual(v['input_trace']['completed'], len(op['actions']))
                self.assertNotIn('Unknown command', v['history'])
                self.assertEqual([e['result'] for e in v['events'] if e['name'] == 'Circle'], ['Success'])
                self.assertEqual(v['before']['outputs'], v['undo']['outputs'])
                self.assertEqual(v['after_script']['outputs'], v['redo']['outputs'])
                self.assertTrue(v['undo_returned'] and v['redo_returned'])
                for stage in ['before', 'after', 'after_script', 'undo', 'redo']:
                    self.assertTrue(v[stage]['guide_unchanged'])
                if 'setup' in op:
                    self.assertEqual(v['setup_input_trace']['errors'], [])
                    self.assertEqual(v['setup_input_trace']['completed'], len(op['setup']))
                for receipt in v['input_trace']['edge_picks']:
                    self.assertTrue(receipt['acknowledged'])
                    edges += 1
                if op['id'] != 'circle-deform-exact-reset':
                    c = v['after_script']['outputs'][0]['curve']
                    degree = c['degree']
                    self.assertEqual(c['control_points'][:degree], c['control_points'][-degree:])
                    self.assertTrue(all(p['weight'] == 1.0 for p in c['control_points']))
                total += 1
        self.assertEqual((total, edges), (47, 3))
        q = json.loads((BASE / 'fixtures/circle_deform_controls.json').read_text())
        n = json.loads((BASE / 'observations/circle_deform_controls_rhino_20261017.json').read_text())
        self.assertEqual(len(q['operations']), 4)
        for op, row in zip(q['operations'], n['results']):
            validate(op)
            v = row['value']
            self.assertEqual(v['input_trace']['errors'], [])
            self.assertEqual(v['input_trace']['completed'], len(op['actions']))
            self.assertEqual([e['result'] for e in v['events'] if e['name'] == 'Circle'], ['Cancel'])
            for stage in ['before', 'after', 'after_script', 'undo', 'redo']:
                self.assertEqual(v[stage]['outputs'], [])
        q = json.loads((BASE / 'fixtures/circle_deform_diagnostics.json').read_text())
        n = json.loads((BASE / 'observations/circle_deform_diagnostics_rhino_20261017.json').read_text())
        self.assertGreaterEqual(len(q['operations']), 5)
        for op, row in zip(q['operations'], n['results']):
            validate(op)
            self.assertEqual(op['id'], row['id'])
            v = row['value']
            if not v['input_trace']['errors'] and 'Unknown command' not in v['history']:
                c = v['after_script']['outputs'][0]['curve']
                self.assertNotEqual(c['control_points'][:c['degree']], c['control_points'][-c['degree']:])

    def test_native_degree_controls_and_complete_input_receipts(self):
        q = json.loads((BASE / 'fixtures/circle_deform_degrees.json').read_text())
        n = json.loads((BASE / 'observations/circle_deform_degrees_rhino_20261017.json').read_text())
        self.assertEqual(len(q['operations']), 10)
        for degree, (op, row) in enumerate(zip(q['operations'], n['results']), 2):
            validate(op)
            self.assertEqual(op['id'], row['id'])
            v = row['value']
            self.assertEqual(v['input_trace']['errors'], [])
            self.assertEqual(v['input_trace']['completed'], len(op['actions']))
            self.assertNotIn('Unknown command', v['history'])
            self.assertEqual([e['result'] for e in v['events'] if e['name'] == 'Circle'], ['Success'])
            self.assertEqual(v['before']['outputs'], v['undo']['outputs'])
            self.assertEqual(v['after']['outputs'], v['after_script']['outputs'])
            self.assertEqual(v['after_script']['outputs'], v['redo']['outputs'])
            c = v['after_script']['outputs'][0]['curve']
            self.assertEqual(c['degree'], degree)
            self.assertEqual(len(c['control_points']), 16 + degree)
            self.assertEqual(c['control_points'][:degree], c['control_points'][-degree:])
            self.assertTrue(all(p['weight'] == 1.0 for p in c['control_points']))

    def test_bounds_point_sources_and_private_standalone_staging(self):
        op = dict(op='circle_deform', id='bounded-circle', live_getters=True,
                  actions=['Deformable', 'Degree=5', 'PointCount=12', 'FitPoints'],
                  curves=[dict(type='point', point=p) for p in [[3, 0, 0], [0, 3, 0], [-3, 0, 0]]],
                  preselect_all=True)
        validate(op)
        for field, value in [('preselect_all', 1), ('live_getters', False),
                             ('actions', ['Degree=100']), ('actions', ['PointCount=1001']),
                             ('actions', ['Degree=3 Delete']), ('actions', ['PointCount=NaN']),
                             ('curves', [dict(type='point', point=[0, 0, float('inf')])]),
                             ('curves', [dict(type='point', point=[0, 0, 0], extra='Delete')]),
                             ('curves', [dict(type='polycurve', segments=[dict(type='point', point=[0, 0, 0])])])]:
            with self.assertRaises(ValueError, msg=(field, value)):
                validate(dict(op, **{field: value}))
        with self.assertRaises(ValueError):
            validate(dict(op, op='circle_around'))
        q = dict(protocol_version=1, iterations=1, operations=[op])
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            stage_helpers(q, BASE, job)
            subprocess.run([sys.executable, '-I', '-c',
                            'import sys,json;sys.path.insert(0,sys.argv[1]);import circle_around_probe;circle_around_probe.validate(json.loads(sys.argv[2]))',
                            str(job), json.dumps(op)], check=True, capture_output=True)
        with mock.patch.dict(os.environ, dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':301'), clear=True), mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            for request in [dict(q, iterations=2), dict(q, iterations=True),
                            dict(q, operations=[op, op]),
                            dict(q, operations=[op, dict(op='line', id='foreign')])]:
                with self.assertRaises((ValueError, OracleProtocolError)):
                    OracleClient(launcher=Path('/bin/true'), settings_scheme='VibocerosOracleCircleDeformUnit').run_rhino(request, 1)
            launch.assert_not_called()
