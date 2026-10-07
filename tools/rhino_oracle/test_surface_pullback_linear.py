"""Native branches, independent witnesses, and retained parameter-speed limits."""
import hashlib
import json
import math
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .surface_pullback_linear_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]


class SurfacePullbackLinearTests(TestCase):
    def test_closed_recipes_validate_before_launch_and_require_owned_idle_document(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, json.loads((ROOT / 'tools/rhino_oracle/fixtures/surface_pullback_linear.json').read_text()))
        self.assertEqual(len(q['operations']), 24)
        for change in (dict(id='x\n_Exit'), dict(id=[]), dict(case=[]), dict(case='_Exit'),
                       dict(extra=True), dict(op='script')):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, operations=[dict(q['operations'][0], **change)]))
        for change in (dict(protocol_version=True), dict(iterations=True), dict(iterations=2),
                       dict(operations=[]), dict(operations=q['operations'] * 2),
                       dict(operations=[q['operations'][0]] * 2)):
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
        for scheme, env in (
            (None, dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':301')),
            ('VibocerosOracleTest', dict(DISPLAY=':1')),
            ('VibocerosOracleTest', dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':302')),
        ):
            with mock.patch.dict(os.environ, env, clear=True), mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError, OracleProtocolError, ValueError)):
                    OracleClient(settings_scheme=scheme).run_rhino(q, 1)
                launch.assert_not_called()

    def test_capture_keeps_all_inputs_singular_failures_and_speed_discrepancies(self):
        capture = json.loads((ROOT / 'tools/rhino_oracle/observations/surface_pullback_linear.json').read_text())
        self.assertEqual(capture['engine_version'], '8.32.26160.13001')
        self.assertEqual([r['id'] for r in capture['results']], [op['id'] for op in request()['operations']])
        failures = []
        for row in capture['results']:
            v = row['value']
            self.assertTrue(v['sources_unchanged'], row['id'])
            self.assertEqual(len(v['samples']), 129)
            self.assertEqual([s['fraction'] for s in v['samples']], [i / 128 for i in range(129)])
            for s in v['samples']:
                self.assertLess(s['reference_error'], 1e-11)
                self.assertAlmostEqual(math.dist(s['reference_surface_point'], s['spatial_point']), s['reference_error'], places=12)
                if v['native_pullback_succeeded']:
                    self.assertLess(s['native_locus_error'], 1e-6)
                    self.assertTrue(math.isfinite(s['native_error']))
                else:
                    self.assertNotIn('native_surface_point', s)
            timing = v['benchmark']
            self.assertEqual(timing['iterations'], 16)
            self.assertGreater(timing['elapsed_ns'], 0)
            self.assertEqual(timing['successes'], 16 if v['native_pullback_succeeded'] else 0)
            if not v['native_pullback_succeeded']:
                failures.append(row['id'])
                self.assertIsNone(v['native_parameter_curve'])
            elif 'two_poles_swapped' in row['id']:
                self.assertGreater(max(s['native_error'] for s in v['samples']), .028)
            else:
                self.assertLess(max(s['native_error'] for s in v['samples']), 1e-11)
        self.assertEqual(failures, ['pullback_linear_sphere_pole_u_forward', 'pullback_linear_sphere_pole_u_reverse',
                                    'pullback_linear_singular_diagonal_forward', 'pullback_linear_singular_diagonal_reverse'])

    def test_capture_provenance_preserves_native_contract_differences(self):
        p = json.loads((ROOT / 'docs/automatic-surface-pullback-provenance.json').read_text())
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['native_sdk_records'], p['native_pullback_successes'], p['source_purity_agreements']), (24, 20, 24))
        self.assertEqual(len(p['retained_native_failures']), 4)
        self.assertEqual(p['retained_native_parameterization_differences'], ['two_poles_swapped_forward', 'two_poles_swapped_reverse'])
        for path, digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest, path)
