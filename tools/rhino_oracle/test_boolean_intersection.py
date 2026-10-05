"""Native BooleanIntersection workflow evidence and private-display guards."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock
from .boolean_intersection_probe import request, run, validate_request
from .client import OracleClient, OracleError, OracleProtocolError

ROOT = Path(__file__).resolve().parents[2]

class BooleanIntersectionTests(TestCase):
    def test_closed_recipes_owned_document_and_private_launch_guards(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/boolean_intersection_command.json').read_text()))
        for change in (dict(id='x\n_Delete'), dict(id=[]), dict(case=[]), dict(case='_Exit'), dict(extra=True)):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, operations=[dict(q['operations'][0], **change)]))
        for change in (dict(protocol_version=True), dict(iterations=True), dict(iterations=2),
                       dict(operations=[]), dict(operations=q['operations']*2),
                       dict(operations=[q['operations'][0]]*2)):
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
        for scheme, display, headless in ((None, ':301', ':301'),
                ('VibocerosOracleIntersection', ':1', None), ('VibocerosOracleIntersection', ':301', ':302')):
            env = dict(DISPLAY=display)
            if headless: env['VIBOCEROS_ORACLE_HEADLESS'] = headless
            with mock.patch.dict(os.environ, env, clear=True), mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError, OracleProtocolError, ValueError)):
                    OracleClient(settings_scheme=scheme).run_rhino(q, 1)
                launch.assert_not_called()

    def test_native_endcommand_modes_metadata_and_transient_selection(self):
        capture = json.loads((ROOT/'tools/rhino_oracle/observations/boolean_intersection_command.json').read_text())
        self.assertEqual(capture['engine_version'], '8.32.26160.13001')
        self.assertEqual([o['id'] for o in request()['operations']], [r['id'] for r in capture['results']])
        rows = {r['id'].removeprefix('boolean_intersection_'): r['value'] for r in capture['results']}
        for case, row in rows.items():
            command = row['command']
            ends = [e for e in command['events'] if e['name']=='BooleanIntersection']
            self.assertEqual(len(ends), 1, case)
            self.assertEqual(ends[0]['objects'], command['after'])
            for output in command['after']:
                if output['source'] is None:
                    self.assertTrue(output['valid']); self.assertTrue(output['solid'])
        self.assertAlmostEqual(rows['three_common']['command']['after'][0]['volume'], 1., places=10)
        self.assertEqual(rows['three_chain']['command']['after'], [dict(r, selected=True) for r in rows['three_chain']['before']])
        self.assertAlmostEqual(rows['two_sets_multi']['command']['after'][0]['volume'], 1.875, places=10)
        for case in ('two_sets','two_sets_reverse','pre_single'):
            output=rows[case]['command']['after'][0]
            source=1 if case=='two_sets_reverse' else 0
            self.assertEqual(output['geometry_text'], 'geometry-'+str(source))
        self.assertFalse(rows['pre_reverse']['command']['after'][0]['selected'])
        self.assertTrue(rows['pre_single']['command']['after'][0]['selected'])
        self.assertEqual(rows['undo_redo']['undo']['after'], rows['undo_redo']['before'])
        self.assertEqual(rows['undo_redo']['redo']['after'], rows['undo_redo']['command']['after'])
        for case in ('cancel_selection','cancel_options','cancel_picked'):
            self.assertEqual(rows[case]['command']['after'],rows[case]['before'])
        self.assertEqual([r['source'] for r in rows['remember']['followup']['after']], [0,1,None])
        self.assertEqual([r['selected'] for r in rows['remember']['followup']['after']], [True,True,False])
        # Cancelled DeleteInput changes are discarded by this command.
        self.assertEqual([r['source'] for r in rows['cancel_options']['followup']['after']], [None])

    def test_capture_provenance(self):
        p = json.loads((ROOT/'docs/boolean-intersection-provenance.json').read_text())
        self.assertEqual(p['command_witnesses'], 32)
        self.assertTrue(p['private_xvfb']); self.assertFalse(p['full_native_parity'])
        for path, expected in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(), expected, path)
