"""Bounded edge SubCrv recipes, native parent purity, and capture provenance."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .subcurve_edge_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]


def read(name):
    return json.loads((ROOT / name).read_text())


class SubcurveEdgeTests(TestCase):
    def test_recipes_are_bounded_and_require_private_idle_empty_context(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, read('tools/rhino_oracle/fixtures/subcurve_edge.json'))
        for change in [dict(case='_Exit'), dict(case=[]), dict(id='x\n_Exit'),
                       dict(extra=True), dict(op='script')]:
            with self.assertRaises(ValueError):
                validate_request(dict(q, operations=[dict(q['operations'][0], **change)]))
        for operations in [[], q['operations'] * 4, [q['operations'][0]] * 2]:
            with self.assertRaises(ValueError):
                validate_request(dict(q, operations=operations))
        rhino = mock.Mock()
        rhino.Commands.Command.InCommand.return_value = True
        with self.assertRaisesRegex(ValueError, 'idle execution'):
            run(q['operations'][0], dict(Rhino=rhino, System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value = False
        rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError, 'empty owned document'):
            run(q['operations'][0], dict(Rhino=rhino, System=mock.Mock()))
        with mock.patch.dict(os.environ, dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':301'), clear=True), \
                mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError, OracleProtocolError, ValueError)):
                OracleClient().run_rhino(q, 1)
            launch.assert_not_called()

    def test_native_edge_outputs_preserve_parent_and_inherit_metadata_or_default_markers(self):
        q = read('tools/rhino_oracle/observations/subcurve_edge.json')
        self.assertEqual(q['engine_version'], '8.32.26160.13001')
        self.assertEqual(len(q['results']), 11)
        curves = markers = stations = 0
        for r in q['results']:
            v = r['value']
            self.assertTrue(v['success'], v['case'])
            self.assertFalse(v['command_active'])
            source = v['before'][0]
            self.assertEqual(source['components'], [['BrepEdge', v['edge_index']]])
            retained = dict(source, components=[])
            self.assertEqual(v['after'][0], retained, v['case'])
            self.assertEqual(v['undo'], [retained])
            self.assertEqual(v['redo'], v['after'])
            self.assertEqual(v['after_script'], v['after'])
            for result in v['after'][1:]:
                self.assertEqual(result['components'], [])
                if result['kind'] == 'curve':
                    curves += 1
                    stations += len(result['samples'])
                    self.assertTrue(result['selected'])
                    for key in ['name', 'layer', 'user_text', 'groups']:
                        self.assertEqual(result[key], source[key])
                else:
                    markers += 1
                    self.assertEqual(result['kind'], 'point')
                    self.assertFalse(result['selected'])
                    self.assertEqual(result['groups'], [])
                    self.assertIsNone(result['name'])
                    self.assertIsNone(result['user_text'])
                    self.assertTrue(result['layer'].endswith('_output'))
        self.assertEqual((curves, markers, stations), (9, 4, 297))
        copy_no = next(r['value'] for r in q['results'] if r['value']['case'] == 'point_copy_no')
        self.assertIn('_Copy=_No', copy_no['macro'])
        self.assertEqual(len(copy_no['after']), 2)

    def test_provenance_binds_complete_raw_capture_and_closed_recipe_family(self):
        p = read('docs/subcurve-edge-provenance.json')
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['recipes'], p['successful_commands'], p['curves'], p['markers'], p['stations']),
                         (11, 11, 9, 4, 297))
        for name, digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / name).read_bytes()).hexdigest(), digest, name)
