"""Private SDK recipes for surface/parameter-curve correspondence."""
import hashlib
import json
import math
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .surface_curve_image_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]


class SurfaceCurveImageTests(TestCase):
    def test_closed_recipes_require_idle_owned_document_and_private_display(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/surface_curve_image.json').read_text()))
        for change in (dict(id='x\n_Exit'), dict(id=[]), dict(case=[]), dict(case='_Exit'),
                       dict(extra=True), dict(op='script')):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, operations=[dict(q['operations'][0], **change)]))
        for change in (dict(protocol_version=True), dict(iterations=True), dict(iterations=2),
                       dict(operations=[]), dict(operations=q['operations']*3),
                       dict(operations=[q['operations'][0]]*2)):
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

    def test_native_capture_keeps_all_sources_and_uniform_and_locus_errors(self):
        capture = json.loads((ROOT/'tools/rhino_oracle/observations/surface_curve_image.json').read_text())
        self.assertEqual(capture['engine_version'], '8.32.26160.13001')
        self.assertEqual([r['id'] for r in capture['results']], [op['id'] for op in request()['operations']])
        for row in capture['results']:
            value = row['value']
            self.assertTrue(value['sources_unchanged'], row['id'])
            self.assertEqual(len(value['samples']), 129)
            self.assertEqual([p['fraction'] for p in value['samples']], [i/128 for i in range(129)])
            for kind in ('error', 'locus_error'):
                self.assertTrue(all(math.isfinite(p[kind]) and p[kind] >= 0 for p in value['samples']))
            self.assertEqual(value['maximum_sample_error'], max(p['error'] for p in value['samples']))
            self.assertEqual(value['maximum_locus_error'], max(p['locus_error'] for p in value['samples']))
            if value['pushup']:
                self.assertLessEqual(value['maximum_locus_error'], value['pushup_tolerance'])
        rows = {r['id']: r['value'] for r in capture['results']}
        seam = rows['surface_image_sphere_seam']
        self.assertGreater(seam['maximum_sample_error'], .006)
        self.assertLess(seam['maximum_locus_error'], 1e-6)
        self.assertEqual(rows['surface_image_warped_offset']['maximum_sample_error'], 1/1024)
        self.assertEqual(rows['surface_image_warped_bump']['maximum_sample_error'], 1/256)

    def test_capture_provenance_keeps_the_parameterization_discrepancy(self):
        p = json.loads((ROOT/'docs/surface-curve-certificate-provenance.json').read_text())
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        self.assertEqual(p['settings_scheme'], 'VibocerosOracleSurfaceImageFinal_20261005')
        self.assertEqual((p['sdk_recipes'], p['pushup_recipes'], p['certified_native_records']), (13, 5, 10))
        self.assertIn('sphere_seam', p['retained_parameterization_differences'])
        for path, digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(), digest, path)
