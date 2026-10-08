"""Circle scale capture ownership, diagnostic geometry and reference evidence."""
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import sys
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .planar_circle_scale_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]


def native_diagnostics(q):
    rows = {}
    for result in q['results']:
        v = result['value']
        scale = v['scale']
        a, b = v['shapes']
        r, s = a['radius']/scale, b['radius']/scale
        d = abs(a['center'][0]-b['center'][0])/scale
        x = (d*d+r*r-s*s)/(2*d)
        h = math.sqrt(r*r-x*x)
        overlap = r*r*math.atan2(h, x) + s*s*math.atan2(h, d-x) - d*h
        expected = {'PlanarUnion': math.pi*(r*r+s*s)-overlap,
                    'PlanarDifference': math.pi*r*r-overlap,
                    'PlanarIntersection': overlap}[v['command_name']]
        after = v['command']['after_script']
        locus = max(min(abs(math.hypot((point[0]-shape['center'][0])/scale,
                                      (point[1]-shape['center'][1])/scale)-shape['radius']/scale)
                        for shape in v['shapes'])
                    for obj in after for edge in obj['edge_samples'] for point in edge)
        rows[v['case']] = dict(normalized_area_error=abs(sum(o['area'] for o in after)/scale**2-expected),
                              normalized_circle_locus_error=locus,
                              normalized_source_area_error=max(abs(obj['area']/scale**2-math.pi*(shape['radius']/scale)**2)
                                                               for obj, shape in zip(v['before'], v['shapes'])))
    return rows


class PlanarCircleScaleTests(TestCase):
    def test_closed_recipes_require_private_empty_idle_execution(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/planar_circle_scale.json').read_text()))
        for change in [dict(case='_Exit'), dict(id='x\n_Exit'), dict(extra=True), dict(op='script')]:
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

    def test_complete_capture_retains_native_geometry_discrepancies(self):
        p = json.loads((ROOT/'docs/planar-circle-scale-provenance.json').read_text())
        q = json.loads((ROOT/'tools/rhino_oracle/observations/planar_circle_scale.json').read_text())
        self.assertEqual((p['recipes'], p['successful_commands']), (27, 27))
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['boundary_agreement_recipes'], p['boundary_diagnostic_recipes']), (6, 21))
        self.assertEqual(len(p['measured_local_native_bidirectional_witness_errors']), 21)
        for name, digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(), digest, name)
        self.assertEqual({r['id'] for r in q['results']}, {o['id'] for o in request()['operations']})
        self.assertEqual(native_diagnostics(q), p['native_diagnostics'])
        for result in q['results']:
            v = result['value']
            self.assertTrue(v['command']['success'])
            self.assertEqual(v['undo']['after_script'], v['before'])
            self.assertEqual(v['redo']['after_script'], v['command']['after_script'])
            for obj in v['command']['after_script']:
                self.assertEqual(len(obj['edge_curves']), obj['edges'])
                self.assertTrue(all(len(points) == 33 for points in obj['edge_samples']))

    def test_fraction_decimal_reference_is_reproducible(self):
        actual = subprocess.check_output([sys.executable, str(ROOT/'tools/numerics/generate_circle_cut_reference.py')])
        self.assertEqual(actual, (ROOT/'crates/viboceros-geometry/src/brep/boolean/circle_cut_reference.json').read_bytes())
        self.assertEqual(len(json.loads(actual)['cases']), 172)
