"""Closed command recipes, native workflows, and private oracle launch guards."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .boolean_union_probe import request, run, validate_request
from .client import OracleClient, OracleError, OracleProtocolError

ROOT = Path(__file__).resolve().parents[2]


class BooleanUnionTests(TestCase):
    def test_closed_recipes_and_owned_document_guards(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/boolean_union_command.json').read_text()))
        for change in (dict(id='x\n_Delete'), dict(id=[]), dict(case=[]),
                       dict(case='_Exit'), dict(op='_Delete'), dict(extra=True)):
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

    def test_private_display_and_preferences_precede_launch(self):
        for scheme, display, headless in ((None, ':301', ':301'),
                ('VibocerosOracleUnion', ':1', None), ('VibocerosOracleUnion', ':301', ':302')):
            env = dict(DISPLAY=display)
            if headless:
                env['VIBOCEROS_ORACLE_HEADLESS'] = headless
            with self.subTest(display=display, scheme=scheme), mock.patch.dict(os.environ, env, clear=True), \
                    mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError, OracleProtocolError, ValueError)):
                    OracleClient(settings_scheme=scheme).run_rhino(request(), 1)
                launch.assert_not_called()

    def test_native_command_end_and_metadata_evidence(self):
        r = json.loads((ROOT/'tools/rhino_oracle/observations/boolean_union_command.json').read_text())
        self.assertEqual([o['id'] for o in request()['operations']], [o['id'] for o in r['results']])
        self.assertEqual(r['engine_version'], '8.32.26160.13001')
        cases = {row['id'].removeprefix('boolean_union_'): row['value'] for row in r['results']}
        failures = {'disjoint': 'Failure', 'contained': 'Nothing', 'equal': 'Failure',
                    'touch_point': 'Nothing', 'single': 'Cancel', 'cancel_picked': 'Cancel',
                    'cancel_selection': 'Cancel', 'cancel_options': 'Cancel'}
        for name, value in cases.items():
            with self.subTest(case=name):
                c = value['command']
                events = [e for e in c['events'] if e['name'] == 'BooleanUnion']
                self.assertEqual([e['result'] for e in events], [failures.get(name, 'Success')])
                self.assertEqual(events[0]['objects'], c['after'])
                # RunScript releases transient postselection after EndCommand.
                self.assertEqual([{k: v for k, v in o.items() if k != 'selected'} for o in c['after']],
                                 [{k: v for k, v in o.items() if k != 'selected'} for o in c['after_script']])
                for row in c['after']:
                    if row['source'] is None:
                        first = 1 if name == 'post_reverse' else 0
                        last = 0 if name == 'post_reverse' else 2 if name in (
                            'three_chain', 'three_shared', 'three_shared_merge', 'open_surface') else 1
                        self.assertEqual(row['name'], 'source-'+str(first))
                        self.assertEqual(row['layer'], first)
                        self.assertEqual(row['color'], [20+first, 40, 60])
                        self.assertEqual(row['attribute_text'], 'attribute-'+str(first))
                        self.assertEqual(row['geometry_text'], 'geometry-'+str(last))
                        self.assertEqual(row['groups'], [first, len(value['before'])])
                        self.assertEqual(row['selected'], name in ('pre_reverse', 'pre_delete_no'))
        for name in ('disjoint', 'contained', 'equal', 'touch_point'):
            self.assertEqual([o['selected'] for o in cases[name]['command']['after']], [True, True])
        for name in ('cancel_selection', 'cancel_options', 'cancel_picked'):
            self.assertEqual(cases[name]['command']['after'], cases[name]['before'])
        for name in ('remember', 'cancel_options'):
            rows = cases[name]['followup']['after']
            self.assertEqual([o['source'] for o in rows], [0, 1, None])
            self.assertEqual([o['selected'] for o in rows], [True, True, False])
        self.assertEqual(cases['undo_redo']['undo']['after'], cases['undo_redo']['before'])
        self.assertEqual(cases['undo_redo']['redo']['after'], cases['undo_redo']['command']['after'])
        # Preserve the successful open/nonmanifold witnesses outside our certificate.
        for name in ('open_surface', 'touch_edge'):
            self.assertFalse(cases[name]['command']['after'][-1]['solid'])

    def test_provenance(self):
        p = json.loads((ROOT/'docs/boolean-union-provenance.json').read_text())
        self.assertEqual(p['command_witnesses'], 32)
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        for path, expected in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(), expected, path)
