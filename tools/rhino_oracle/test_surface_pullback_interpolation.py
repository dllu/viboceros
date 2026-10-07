"""Closed native evidence for certified derivative-free pullback fitting."""
import hashlib
import json
import math
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .surface_pullback_interpolation_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]


class SurfacePullbackInterpolationTests(TestCase):
    def test_recipes_validate_before_launch_and_require_private_idle_owned_document(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, json.loads((ROOT / 'tools/rhino_oracle/fixtures/surface_pullback_interpolation.json').read_text()))
        self.assertEqual(len(q['operations']), 10)
        for change in (dict(id='x\n_Exit'), dict(id=[]), dict(case=[]), dict(case='_Exit'),
                       dict(extra=True), dict(op='script')):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, operations=[dict(q['operations'][0], **change)]))
        for change in (dict(protocol_version=True), dict(iterations=True), dict(iterations=2),
                       dict(operations=[]), dict(operations=q['operations'] * 4),
                       dict(operations=[q['operations'][0]] * 2)):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, **change))
        rhino = mock.Mock()
        rhino.Commands.Command.InCommand.return_value = 1
        with self.assertRaisesRegex(ValueError, 'idle execution'):
            run(q['operations'][0], dict(Rhino=rhino))
        rhino.Commands.Command.InCommand.return_value = 0
        rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError, 'empty owned document'):
            run(q['operations'][0], dict(Rhino=rhino))
        for scheme, env in (
            (None, dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':301')),
            ('VibocerosOracleTest', dict(DISPLAY=':1')),
            ('VibocerosOracleTest', dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':302')),
        ):
            with mock.patch.dict(os.environ, env, clear=True), mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError, OracleProtocolError, ValueError)):
                    OracleClient(settings_scheme=scheme).run_rhino(q, 1)
                launch.assert_not_called()

    def test_sources_and_locus_errors_remain_separate_from_native_parameter_speed(self):
        capture = json.loads((ROOT / 'tools/rhino_oracle/observations/surface_pullback_interpolation.json').read_text())
        self.assertEqual([r['id'] for r in capture['results']], [op['id'] for op in request()['operations']])
        successes = 0
        for row in capture['results']:
            v = row['value']
            self.assertTrue(v['sources_unchanged'])
            self.assertEqual(len(v['samples']), 129)
            self.assertEqual([s['fraction'] for s in v['samples']], [i / 128 for i in range(129)])
            for s in v['samples']:
                self.assertLess(s['reference_error'], 1e-11)
                self.assertAlmostEqual(math.dist(s['reference_surface_point'], s['spatial_point']), s['reference_error'], places=12)
            if v['native_pullback_succeeded']:
                successes += 1
                self.assertLess(max(s['native_locus_error'] for s in v['samples']), v['limit'])
                self.assertGreater(max(s['native_error'] for s in v['samples']), .1)
            else:
                self.assertIn('singular_cubic', row['id'])
                self.assertIsNone(v['native_parameter_curve'])
                self.assertTrue(all('native_error' not in s for s in v['samples']))
        self.assertEqual(successes, 8)

    def test_provenance_retains_source_definitions_and_native_failure_diagnostics(self):
        p = json.loads((ROOT / 'docs/derivative-free-pullback-provenance.json').read_text())
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['native_sdk_records'],p['native_pullback_successes'],p['source_purity_agreements']),(10,8,10))
        self.assertEqual(p['retained_native_failures'], ['singular_cubic_forward','singular_cubic_reverse'])
        for path,digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),digest,path)
