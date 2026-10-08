"""Owned scale captures and independent exact cut reference provenance."""
import hashlib
import json
import math
import os
import subprocess
import sys
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .planar_boolean_scale_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]


class PlanarBooleanScaleTests(TestCase):
    def test_bounded_recipes_and_owned_idle_execution(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, json.loads((ROOT / 'tools/rhino_oracle/fixtures/planar_boolean_scale.json').read_text()))
        for change in [dict(case='_Exit'), dict(case=[]), dict(id='x\n_Exit'), dict(extra=True), dict(op='script')]:
            with self.assertRaises(ValueError):
                validate_request(dict(q, operations=[dict(q['operations'][0], **change)]))
        rhino = mock.Mock()
        rhino.Commands.Command.InCommand.return_value = True
        with self.assertRaisesRegex(ValueError, 'idle execution'):
            run(q['operations'][0], dict(Rhino=rhino, System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value = False
        rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError, 'empty owned document'):
            run(q['operations'][0], dict(Rhino=rhino, System=mock.Mock()))
        with mock.patch.dict(os.environ, dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':301'), clear=True), mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError, OracleProtocolError, ValueError)):
                OracleClient().run_rhino(q, 1)
            launch.assert_not_called()

    def test_capture_history_curves_and_measured_native_area_discrepancy(self):
        p = json.loads((ROOT / 'docs/planar-boolean-scale-provenance.json').read_text())
        q = json.loads((ROOT / 'tools/rhino_oracle/observations/planar_boolean_scale.json').read_text())
        self.assertEqual((p['recipes'], p['successful_commands']), (27, 27))
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        for name, digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / name).read_bytes()).hexdigest(), digest, name)
        self.assertEqual({r['id'] for r in q['results']}, {o['id'] for o in request()['operations']})
        errors, source_errors = [], []
        for r in q['results']:
            v = r['value']
            after = v['command']['after_script']
            self.assertTrue(v['command']['success'])
            self.assertEqual(v['undo']['after_script'], v['before'])
            self.assertEqual(v['redo']['after_script'], after)
            scale = v['scale']
            intersection = math.sqrt(3.75) + 8 * math.asin(.25) if '_strip_' in v['case'] else 2 * math.pi
            rectangle = 6 if '_strip_' in v['case'] else 18
            disk = 4 * math.pi
            expected = {'PlanarUnion': disk + rectangle - intersection, 'PlanarIntersection': intersection,
                        'PlanarDifference': (rectangle if '_first_polygon_' in v['case'] else disk) - intersection}[v['command_name']]
            errors.append(abs(sum(o['area'] for o in after) / scale**2 - expected))
            source_errors.extend(abs(o['area'] / scale**2 - (disk if shape['kind'] == 'disk' else rectangle)) for shape, o in zip(v['shapes'], v['before']))
            for o in after:
                self.assertEqual(len(o['edge_curves']), o['edges'])
                self.assertTrue(all(len(points) == 33 for points in o['edge_samples']))
        self.assertAlmostEqual(max(errors), p['measured_native_normalized_area_error_max'], places=14)
        self.assertAlmostEqual(max(source_errors), p['measured_native_normalized_source_area_error_max'], places=14)

    def test_fraction_reference_is_reproducible(self):
        actual = subprocess.check_output([sys.executable, str(ROOT / 'tools/numerics/generate_planar_cut_reference.py')])
        expected = (ROOT / 'crates/viboceros-geometry/src/brep/boolean/planar_mixed/cuts/reference.json').read_bytes()
        self.assertEqual(actual, expected)
        self.assertEqual(len(json.loads(actual)['cases']), 206)
