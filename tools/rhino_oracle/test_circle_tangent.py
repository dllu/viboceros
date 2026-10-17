"""Tangent getter input receipts and calibration use inputs independently of output."""
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from unittest import TestCase, mock
from .circle_around_probe import validate
from .structural_sdk import stage_helpers
from .client import OracleClient, OracleProtocolError

BASE = Path(__file__).parent


class CircleTangentTests(TestCase):
    def test_complete_native_contacts_history_and_click_receipts(self):
        contacts = 0
        for profile, count in [('getters', 10), ('special', 6), ('curves',5), ('spatial',6)]:
            q = json.loads((BASE / ('fixtures/circle_tangent_' + profile + '.json')).read_text())
            n = json.loads((BASE / ('observations/circle_tangent_' + profile + '_rhino_20261017.json')).read_text())
            self.assertEqual(len(q['operations']), count)
            self.assertEqual(len(n['results']), count)
            for op, row in zip(q['operations'], n['results']):
                validate(op)
                self.assertEqual(op['id'], row['id'])
                v = row['value']
                t = v['input_trace']
                self.assertEqual(t['errors'], [])
                self.assertEqual(t['completed'], len(op['actions']))
                self.assertNotIn('Unknown command', v['history'])
                self.assertEqual([e['result'] for e in v['events'] if e['name'] == 'Circle'], ['Success' if v['after_script']['outputs'] else 'Cancel'])
                self.assertEqual(v['before']['outputs'], v['undo']['outputs'])
                self.assertEqual(v['after_script']['outputs'], v['redo']['outputs'])
                if v['after_script']['outputs']:self.assertTrue(v['undo_returned'] and v['redo_returned'])
                for stage in ['before', 'after', 'after_script', 'undo', 'redo']:
                    self.assertTrue(v[stage]['guide_unchanged'])
                for c in t.get('clicks', []):
                    self.assertTrue(c['motion_acknowledged'] and c['click_acknowledged'])
                    a = op['actions'][c['action']]
                    self.assertEqual(c['aim'], a['pick'])
                    self.assertEqual(c['curve'], a['curve'])
                    self.assertEqual(len(c['projected_point']), 3)
                    self.assertIsInstance(c['projected_parameter'], float)
                    contacts += 1
        self.assertEqual(contacts, 44)

    def test_diagnostic_attempt_and_clean_cancel_remain_distinct(self):
        q = json.loads((BASE / 'fixtures/circle_tangent_initial.json').read_text())
        n = json.loads((BASE / 'observations/circle_tangent_initial_rhino_20261017.json').read_text())
        for op, row in zip(q['operations'], n['results']):
            validate(op)
            if op['id'] == 'circle-tangent-fixed':
                self.assertTrue(row['value']['input_trace']['errors'])
            if op['id'] == 'circle-tangent-cancel':
                self.assertEqual(row['value']['input_trace']['errors'], [])
                for stage in ['before', 'after', 'after_script', 'undo', 'redo']:
                    self.assertEqual(row['value'][stage]['outputs'], [])

    def test_bounded_location_inputs_and_standalone_helpers(self):
        op = json.loads((BASE / 'fixtures/circle_tangent_getters.json').read_text())['operations'][0]
        for a in [dict(curve=0), dict(curve=0,pick=[1,2,float('inf')]), dict(curve=True,pick=[1,2,3]), dict(curve=0,pick=[1,2,3],extra='Delete')]:
            with self.assertRaises(ValueError):
                validate(dict(op, actions=['Tangent',a]))
        with self.assertRaises(ValueError):
            validate(dict(op,op='circle_deform'))
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            stage_helpers(dict(protocol_version=1,iterations=1,operations=[op]),BASE,job)
            subprocess.run([sys.executable,'-I','-c','import sys,json;sys.path.insert(0,sys.argv[1]);import circle_around_probe;circle_around_probe.validate(json.loads(sys.argv[2]))',str(job),json.dumps(op)],check=True,capture_output=True)

    def test_private_dedicated_tangent_capture_rejects_incompatible_input_protocols(self):
        q = json.loads((BASE / 'fixtures/circle_tangent_getters.json').read_text())
        op = q['operations'][0]
        around = json.loads((BASE / 'fixtures/circle_around_command.json').read_text())['operations'][0]
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True), mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            for request in [dict(q,iterations=2),dict(q,iterations=True),dict(q,operations=[op,op]),dict(q,operations=[op,around])]:
                with self.assertRaises((ValueError,OracleProtocolError)):
                    OracleClient(launcher=Path('/bin/true'),settings_scheme='VibocerosOracleCircleTangentUnit').run_rhino(request,1)
            launch.assert_not_called()
