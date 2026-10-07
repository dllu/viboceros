"""Independent native getter captures and the Rust/Python range adapter."""
import hashlib
import json
import math
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .uv_subcurve_input_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]


def read(path):
    return json.loads((ROOT / path).read_text())


class UvSubcurveInputTests(TestCase):
    def test_closed_recipes_require_owned_idle_context_and_private_display(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, read('tools/rhino_oracle/fixtures/uv_subcurve_input_command.json'))
        for change in (dict(id='x\n_Exit'), dict(id=[]), dict(case=[]), dict(case='_Exit'),
                       dict(extra=True), dict(op='script')):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, operations=[dict(q['operations'][0], **change)]))
        for change in (dict(protocol_version=True), dict(iterations=True), dict(iterations=2),
                       dict(operations=[]), dict(operations=q['operations'] * 2)):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, **change))
        rhino = mock.Mock()
        rhino.Commands.Command.InCommand.return_value = 1
        with self.assertRaisesRegex(ValueError, 'idle execution'):
            run(q['operations'][0], dict(Rhino=rhino, System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value = 0
        rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError, 'empty owned document'):
            run(q['operations'][0], dict(Rhino=rhino, System=mock.Mock()))
        with mock.patch.dict(os.environ, dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':301'), clear=True), mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError, OracleProtocolError, ValueError)):
                OracleClient().run_rhino(q, 1)
            launch.assert_not_called()

    def test_native_transient_defaults_source_purity_and_history(self):
        capture = read('tools/rhino_oracle/observations/uv_subcurve_input_command.json')
        self.assertEqual(len(capture['results']), 14)
        curves = points = 0
        for row in capture['results']:
            v = row['value']
            self.assertTrue(v['success'], row['id'])
            self.assertFalse(v['command_active'])
            originals = {o['source']: o for o in v['before']}
            for o in v['after']:
                if o['source'] is not None:
                    self.assertEqual(o, dict(originals[o['source']], selected=o['selected']))
                else:
                    self.assertTrue(o['selected'])
                    if o['name'] is None:
                        self.assertEqual(o['layer'], 'Default')
                        self.assertEqual(o['groups'], [])
                        self.assertIsNone(o['user_text'])
                    if o['kind'] == 'curve':
                        curves += 1
                        self.assertEqual(len(o['samples']), 33)
                    else:
                        points += 1
            self.assertEqual(len(v['undo']), len(v['before']))
            self.assertTrue(all(not o['selected'] for o in v['undo']))
            self.assertEqual(v['groups_after'], v['groups_undo'])
            self.assertEqual(v['groups_after'], v['groups_redo'])
            self.assertTrue(all(not o['selected'] for o in v['redo'] if o['source'] is not None))
        self.assertEqual((curves, points), (24, 2))

    def test_local_sources_station_comparison_and_retained_closed_parameter_speed(self):
        native = read('tools/rhino_oracle/observations/uv_subcurve_input_command.json')['results']
        fixtures = read('tools/rhino_oracle/fixtures/uv_subcurve_input_local.json')['operations']
        local = read('docs/uv-subcurve-input-local.json')['results']
        self.assertEqual((len(fixtures), len(local)), (14, 14))
        for n, f, l in zip(native, fixtures, local):
            self.assertEqual((n['id'], l['id']), (f['id'], f['id']))
            for source, item in zip(n['value']['before'][1:], f['inputs']):
                if source['kind'] == 'curve': self.assertEqual(source['definition'], item['definition'])
                else: self.assertEqual(source['point'], item['point'])
            if n['id'].endswith('clear'):
                self.assertFalse(f['inputs'][0]['selected'])
                self.assertEqual(f['inputs'][0]['subcurves'], [])
            else: self.assertEqual(f['inputs'][0]['subcurves'], n['value']['ranges'])
            a = [o for o in n['value']['after'] if o['source'] is None]
            b = [o for o in l['value']['after'] if o['source'] is None]
            self.assertEqual(len(a), len(b))
            for expected, actual in zip(a, b):
                self.assertEqual(expected['kind'], actual['kind'])
                self.assertEqual(len(expected['groups']), actual['group_count'])
                if expected['kind'] == 'point':
                    self.assertLess(math.dist(expected['point'], actual['point']), 1e-6)
                    continue
                differences = [math.dist(p, q) for p, q in zip(expected['samples'], actual['samples'])]
                if n['id'].endswith('create_closed') and expected['name'] is None:
                    self.assertAlmostEqual(max(differences), .50625, places=10)
                    self.assertEqual((expected['definition']['degree'], actual['definition']['degree']), (1, 1))
                    for p, q in zip(expected['definition']['control_points'], actual['definition']['control_points']):
                        self.assertEqual((p['weight'], q['weight']), (1., 1.))
                        self.assertLess(math.dist(p['point'], q['point']), 1e-6)
                else: self.assertLess(max(differences), 1e-6, n['id'])
            self.assertEqual(l['value']['groups_after'], len(n['value']['groups_after']))
            self.assertEqual(l['value']['groups_after'], l['value']['groups_after_undo'])

    def test_provenance_retains_unmodified_raw_evidence(self):
        p = read('docs/uv-subcurve-input-provenance.json')
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        self.assertEqual(p['settings_scheme'], 'VibocerosOracleUVSubcurveClear20261007')
        self.assertEqual((p['recipes'], p['native_curves'], p['native_points']), (14, 24, 2))
        self.assertAlmostEqual(p['retained_closed_parameter_speed_discrepancy'], .50625)
        for path, digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest, path)
