"""Immutable native interpolation evidence and complete qualification guards."""
from tools.verify_provenance import reconstruct_measurement, verify_measurement
import copy
import gzip
import hashlib
import json
import os
from pathlib import Path
import unittest
from unittest import mock
from . import face_create_probe as probe
from .client import OracleClient, OracleProtocolError
from .report_subd_interpolation import GROUPS, ROOT, summarize
from .structural_sdk import validate_operations

HERE = Path(__file__).resolve().parent


class SubDInterpolationTests(unittest.TestCase):
    def test_fixed_recipes_native_history_and_archived_producers(self):
        factories = [probe.subd_interp_request, probe.subd_interp_draft_request,
                     probe.subd_interp_order_request]
        for (name, count), factory in zip(GROUPS, factories):
            path = HERE / 'fixtures' / (name + '.json')
            request = json.loads(path.read_text())
            self.assertEqual(request, factory())
            validate_operations(request)
            response = json.loads((HERE / 'observations' / (name + '.json')).read_text())
            self.assertEqual(len(response['results']), count)
            self.assertEqual(response['provenance']['request_sha256'],
                             hashlib.sha256(path.read_bytes()).hexdigest())
            self.assertEqual([r['id'] for r in response['results']],
                             [o['id'] for o in request['operations']])
            for source, sha in response['provenance']['sources'].items():
                data = (HERE / source).read_bytes()
                if hashlib.sha256(data).hexdigest() != sha:
                    data = gzip.decompress((HERE / 'observations/source_snapshots' / (sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(), sha, source)
            for row in response['results']:
                value = row['value']
                self.assertEqual(value['input'], probe.SCENES[row['id']])
                self.assertTrue(value['success'])
                self.assertEqual(value['after_script'], value['redo'])
                self.assertNotIn('Unknown command:', value['history'])
                self.assertEqual(len([e for e in value['events'] if e['name'] == '3DFace']), 1)
                if any(isinstance(p, dict) for p in value['input']['points']):
                    self.assertGreater(value['input_trace']['requests'], 0)
                    self.assertTrue(all(p.startswith('Pick point') for p in value['input_trace']['prompts']))
                for output in value['after_script']:
                    if 'subd' in output:
                        self.assertTrue(output['subd']['valid'])
                        self.assertIn('surface_mesh', output['subd'])

    def test_physical_inputs_require_the_exact_dedicated_sequence(self):
        with mock.patch.dict(os.environ, {'DISPLAY': ':999', 'VIBOCEROS_ORACLE_HEADLESS': ':999'}), mock.patch('subprocess.Popen') as spawn:
            client = OracleClient(settings_scheme='VibocerosOracleSubDInterpolationTest20261021')
            for factory in [probe.subd_interp_draft_request, probe.subd_interp_order_request]:
                for kind in ['reverse', 'mix', 'repeat', 'duplicate', 'subset']:
                    request = factory()
                    if kind == 'reverse':
                        request['operations'].reverse()
                    elif kind == 'mix':
                        request['operations'].append(probe.subd_interp_request()['operations'][0])
                    elif kind == 'repeat':
                        request['iterations'] = 2
                    elif kind == 'duplicate':
                        request['operations'].append(copy.deepcopy(request['operations'][0]))
                    else:
                        request['operations'] = request['operations'][:-1]
                    with self.assertRaises(OracleProtocolError):
                        client.run_rhino(request)
            spawn.assert_not_called()

    def lines(self):
        report = json.loads((ROOT / 'docs/subd-interpolation-qualification.json').read_text())
        return report, ['SUBD_INTERP_' + stage.upper() + ' ' + json.dumps(row)
                        for stage, rows in report['stages'].items() for row in rows]

    def test_complete_historical_qualification(self):
        report, lines = self.lines()
        self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(lines)), report)
        self.assertEqual(report['recipes'], 39)
        self.assertEqual(report['model_epsilon'], 1e-6)
        self.assertFalse(report['full_native_parity'])
        self.assertFalse(report['performance_qualified'])
        self.assertGreater(verify_measurement(report), 0)

    def test_missing_failed_or_hidden_outputs_cannot_qualify(self):
        report, lines = self.lines()
        for invalid in [lines[:-1], lines + lines[-1:], lines[::-1]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(invalid))
        for key, bad in [('qualified', False), ('maximum_sampled_surface_error', float('nan')),
                         ('maximum_control_or_limit_point_error', True),
                         ('maximum_sampled_surface_error', 1.1e-6), ('faces', 0),
                         ('surface_sampled', False)]:
            row = copy.deepcopy(report['stages']['command'][0])
            row['check']['objects'][0][key] = bad
            invalid = lines[:]
            invalid[0] = 'SUBD_INTERP_COMMAND ' + json.dumps(row)
            with self.assertRaises(ValueError):
                summarize('\n'.join(invalid))
        for key in ['grouped_history', 'attributes_selection_and_identity']:
            row = copy.deepcopy(report['stages']['command'][0])
            (row if key == 'grouped_history' else row['check'])[key] = False
            invalid = lines[:]
            invalid[0] = 'SUBD_INTERP_COMMAND ' + json.dumps(row)
            with self.assertRaises(ValueError):
                summarize('\n'.join(invalid))
        row = next(copy.deepcopy(r) for r in report['stages']['command']
                   if r['id'] == 'subd_interp_append_disconnected')
        row['check']['objects'].pop()
        invalid = [line if not line.startswith('SUBD_INTERP_COMMAND ') or json.loads(line.split(' ', 1)[1])['id'] != row['id']
                   else 'SUBD_INTERP_COMMAND ' + json.dumps(row) for line in lines]
        with self.assertRaises(ValueError):
            summarize('\n'.join(invalid))


if __name__ == '__main__':
    unittest.main()
