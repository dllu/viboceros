"""Retained native sources for certified pullbacks with endpoint constraints."""
import hashlib
import json
import math
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .surface_pullback_endpoints_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]


class SurfacePullbackEndpointsTests(TestCase):
    def test_closed_recipes_validate_before_launch_and_require_owned_idle_document(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, json.loads((ROOT / 'tools/rhino_oracle/fixtures/surface_pullback_endpoints.json').read_text()))
        for change in (dict(id='x\n_Exit'), dict(id=[]), dict(case=[]), dict(case='_Exit'),
                       dict(extra=True), dict(op='script')):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, operations=[dict(q['operations'][0], **change)]))
        for change in (dict(protocol_version=True), dict(iterations=True), dict(iterations=2),
                       dict(operations=[]), dict(operations=q['operations'] * 5),
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

    def test_native_records_keep_sources_endpoints_and_parameter_speed_discrepancy(self):
        capture = json.loads((ROOT / 'tools/rhino_oracle/observations/surface_pullback_endpoints.json').read_text())
        self.assertEqual(capture['engine_version'], '8.32.26160.13001')
        self.assertEqual([r['id'] for r in capture['results']], [op['id'] for op in request()['operations']])
        for row in capture['results']:
            v = row['value']
            self.assertTrue(v['sources_unchanged'], row['id'])
            self.assertTrue(v['native_pullback_succeeded'], row['id'])
            self.assertEqual(len(v['samples']), 129)
            self.assertEqual([s['fraction'] for s in v['samples']], [i / 128 for i in range(129)])
            for key in ('error', 'locus_error'):
                self.assertTrue(all(math.isfinite(s[key]) and s[key] >= 0 for s in v['samples']))
            self.assertLess(max(s['locus_error'] for s in v['samples']), v['limit'])
            self.assertEqual([s['parameter_point'] for s in v['endpoint_samples']], v['endpoints'])
            for s in v['endpoint_samples']:
                self.assertAlmostEqual(math.dist(s['surface_point'], s['spatial_point']), s['error'], places=12)
                self.assertLessEqual(s['error'], v['limit'])
            paired_error = max(s['error'] for s in v['samples'])
            if 'planar' in row['id']:
                self.assertGreater(paired_error, .085)
            else:
                self.assertNotEqual(v['endpoints'][0], v['endpoints'][1])
                self.assertLess(math.dist(v['endpoint_samples'][0]['spatial_point'],
                                          v['endpoint_samples'][1]['spatial_point']), 1e-11)
                self.assertLess(paired_error, 1e-11)

    def test_provenance_preserves_complete_inputs_and_native_contract_limits(self):
        p = json.loads((ROOT / 'docs/constrained-surface-pullback-provenance.json').read_text())
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['native_sdk_records'], p['native_pullback_successes'],
                          p['source_purity_agreements']), (8, 8, 8))
        self.assertEqual(p['retained_native_parameterization_differences'], ['planar_forward', 'planar_reverse'])
        for path, digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest, path)
