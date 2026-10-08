"""BooleanSplit closed recipes, measured policies, and native provenance."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .boolean_split_probe import request, run, validate_request
from .client import OracleClient, OracleError, OracleProtocolError

ROOT = Path(__file__).resolve().parents[2]


def read(name):
    return json.loads((ROOT / name).read_text())


class BooleanSplitTests(TestCase):
    def test_closed_recipes_require_private_idle_empty_context(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, read('tools/rhino_oracle/fixtures/boolean_split_command.json'))
        for update in [dict(case='_Exit'), dict(case=[]), dict(id='x\n_Exit'),
                       dict(op='script'), dict(extra=True)]:
            with self.assertRaises(ValueError):
                validate_request(dict(q, operations=[dict(q['operations'][0], **update)]))
        for update in [dict(iterations=True), dict(iterations=2), dict(protocol_version=True),
                       dict(extra=True), dict(operations=[]), dict(operations=q['operations'] * 3)]:
            with self.assertRaises(ValueError):
                validate_request(dict(q, **update))
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

    def test_native_outputs_keep_cutters_preserve_unchanged_targets_and_conserve_volume(self):
        q = read('tools/rhino_oracle/observations/boolean_split_command.json')
        self.assertEqual(len(q['results']), 28)
        success = outputs = 0
        for r in q['results']:
            v = r['value']
            before, after = v['before'], v['command']['after']
            self.assertEqual([dict(o, selected=o['selected'] if o['source'] is None else False) for o in after],
                             v['command']['after_script'])
            produced = [o for o in after if o['source'] is None]
            if not v['command']['success']:
                self.assertEqual(produced, [])
                self.assertEqual([dict(o, selected=False) for o in after],
                                 [dict(o, selected=False) for o in before])
                continue
            success += 1
            outputs += len(produced)
            for i in v['second']:
                if i in v['first']:
                    continue
                retained = next(o for o in after if o['source'] == i)
                self.assertEqual(dict(retained, selected=False), dict(before[i], selected=False))
                self.assertTrue(retained['selected'])
                self.assertFalse(next(o for o in v['command']['after_script'] if o['source'] == i)['selected'])
            for i in v['first']:
                pieces = [o for o in produced if o['name'] == before[i]['name']]
                if not pieces:
                    self.assertIn(dict(before[i], selected=False), after)
                    continue
                self.assertAlmostEqual(sum(o['volume'] for o in pieces), before[i]['volume'], places=7)
                for piece in pieces:
                    self.assertTrue(piece['valid'] and piece['solid'])
                    self.assertEqual(piece['selected'], v['pre'])
                    for key in ['name', 'layer', 'color', 'groups', 'attribute_text']:
                        self.assertEqual(piece[key], before[i][key])
                    self.assertIn(piece['geometry_text'], [None, before[i]['geometry_text']])
            self.assertFalse(any(o['selected'] for o in v['undo']['after']))
            self.assertFalse(any(o['selected'] for o in v['redo']['after'] if o['source'] is not None))
            self.assertEqual([dict(o, selected=False) for o in v['redo']['after']],
                             [dict(o, selected=False) for o in after])
        self.assertEqual((success, outputs), (17, 64))

    def test_cancelled_options_survive_both_getters_and_disconnected_descendants_drop_geometry_text(self):
        q = read('tools/rhino_oracle/observations/boolean_split_command.json')
        cases = {r['value']['case']: r['value'] for r in q['results']}
        for case in ['slab_cancel_options', 'slab_cancel_cutters_options']:
            v = cases[case]
            self.assertFalse(v['command']['success'])
            self.assertFalse(any(o['selected'] for o in v['command']['after']))
            self.assertTrue(v['followup']['success'])
            self.assertTrue(any(o['source'] == 0 for o in v['followup']['after']))
        for case in ['slab', 'two_slabs', 'cross_xy', 'cross_xy_reverse', 'duplicate_cutters']:
            pieces = [o for o in cases[case]['command']['after'] if o['source'] is None]
            self.assertEqual(sum(o['geometry_text'] is not None for o in pieces), 1)
        self.assertTrue(all(o['geometry_text'] == 'geometry-0'
                            for o in cases['overlap_cutters']['command']['after'] if o['source'] is None))

    def test_provenance_binds_capture_and_declares_order_and_input_limits(self):
        p = read('docs/boolean-split-provenance.json')
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['recipes'], p['successful_commands'], p['retained_no_split_failures'],
                          p['retained_cancellations'], p['followups'], p['main_outputs']),
                         (28, 17, 8, 3, 2, 64))
        for path, digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(), digest, path)
