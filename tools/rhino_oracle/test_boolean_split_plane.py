"""Finite-sheet BooleanSplit captures and closed recipe validation."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock
from .boolean_split_plane_probe import request, validate_request, run
from .client import OracleClient, OracleError, OracleProtocolError
ROOT = Path(__file__).resolve().parents[2]


def read(name):
    return json.loads((ROOT/name).read_text())


class BooleanSplitPlaneTests(TestCase):
    def test_owned_recipes_are_closed_and_require_private_idle_empty_context(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, read('tools/rhino_oracle/fixtures/boolean_split_plane.json'))
        for change in [dict(case='_Exit'), dict(case=[]), dict(id='x\n_Exit'), dict(extra=True), dict(op='script')]:
            with self.assertRaises(ValueError):
                validate_request(dict(q, operations=[dict(q['operations'][0], **change)]))
        for change in [dict(iterations=True), dict(iterations=2), dict(operations=[]), dict(extra=True)]:
            with self.assertRaises(ValueError):
                validate_request(dict(q, **change))
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

    def test_native_capped_pieces_keep_cutters_metadata_volume_and_history(self):
        q = read('tools/rhino_oracle/observations/boolean_split_plane.json')
        success = outputs = 0
        self.assertEqual(len(q['results']), 19)
        for r in q['results']:
            v = r['value']
            before, after = v['before'], v['command']['after_script']
            pieces = [o for o in after if o['source'] is None]
            self.assertFalse(any(o['selected'] for o in after if o['source'] is not None))
            for i in v['second']:
                retained = next(o for o in after if o['source'] == i)
                self.assertEqual(retained, dict(before[i], selected=False))
            if not v['command']['success']:
                self.assertEqual(after, [dict(o, selected=False) for o in before])
                self.assertEqual(pieces, [])
                continue
            success += 1
            outputs += len(pieces)
            self.assertAlmostEqual(sum(o['volume'] for o in pieces), before[0]['volume'], places=7)
            for piece in pieces:
                self.assertTrue(piece['valid'] and piece['solid'])
                self.assertEqual(piece['selected'], v['pre'])
                for key in ['name', 'layer', 'color', 'attribute_text', 'groups']:
                    self.assertEqual(piece[key], before[0][key])
                self.assertIn(piece['geometry_text'], [None, before[0]['geometry_text']])
            self.assertEqual(v['redo']['after_script'], after)
            self.assertEqual(v['undo']['after_script'], [dict(o, selected=False) for o in before])
        self.assertEqual((success, outputs), (14, 39))

    def test_finite_extent_joint_coverage_and_cut_order_are_measured(self):
        cases = {r['value']['case']: r['value'] for r in read('tools/rhino_oracle/observations/boolean_split_plane.json')['results']}
        self.assertFalse(cases['partial']['command']['success'])
        self.assertFalse(cases['internal']['command']['success'])
        self.assertFalse(cases['coplanar_gap']['command']['success'])
        self.assertTrue(cases['coplanar_halves']['command']['success'])
        pieces = lambda c: [o for o in cases[c]['command']['after_script'] if o['source'] is None]
        self.assertEqual(len(pieces('partial_then_full')), 2)
        self.assertEqual(len(pieces('full_then_partial')), 3)
        self.assertEqual([o['faces'] for o in pieces('coplanar_halves')], [7, 7])
        for case in ['mixed_closed_first', 'mixed_plane_first']:
            self.assertEqual(len(pieces(case)), 6)
            self.assertEqual(sum(o['geometry_text'] is not None for o in pieces(case)), 2)

    def test_provenance_binds_original_capture_and_states_limits(self):
        p = read('docs/boolean-split-plane-provenance.json')
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['recipes'], p['successful_commands'], p['no_split_failures'], p['outputs']), (19, 14, 5, 39))
        for path, digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(), digest, path)
