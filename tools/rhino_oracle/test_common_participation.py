"""Closed common-intersection recipes and retained owned-document evidence."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .common_participation_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]


class CommonParticipationTests(TestCase):
    def capture(self):
        return json.loads((ROOT/'tools/rhino_oracle/observations/common_participation.json').read_text())

    def test_closed_recipes_require_idle_owned_document_and_private_display(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/common_participation.json').read_text()))
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
            run(q['operations'][0], dict(Rhino=rhino, System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value = 0
        rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError, 'empty owned document'):
            run(q['operations'][0], dict(Rhino=rhino, System=mock.Mock()))
        for scheme, env in (
            (None, dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':301')),
            ('VibocerosOracleTest', dict(DISPLAY=':1')),
            ('VibocerosOracleTest', dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':302')),
        ):
            with mock.patch.dict(os.environ, env, clear=True), mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError, OracleProtocolError, ValueError)):
                    OracleClient(settings_scheme=scheme).run_rhino(q, 1)
                launch.assert_not_called()

    def test_capture_retains_all_closed_commands_events_and_history(self):
        capture = self.capture()
        self.assertEqual(capture['engine_version'], '8.32.26160.13001')
        self.assertEqual([r['id'] for r in capture['results']], [r['id'] for r in request()['operations']])
        histories = 0
        for row in capture['results']:
            value = row['value']
            command = value['command']
            events = [e for e in command['events'] if e['name'] == 'BooleanIntersection']
            self.assertEqual(len(events), 1, row['id'])
            self.assertIn(events[0]['result'], ('Success', 'Failure', 'Nothing'))
            self.assertEqual(events[0]['objects'], command['after'])
            self.assertNotIn('Unknown command:', command['history'])
            for g in value['before'] + command['after']:
                self.assertTrue(g['valid'] and g['solid'], row['id'])
                self.assertEqual((g['faces'], g['edges']), (6, 12))
                self.assertEqual(len(g['face_regions']), g['faces'])
                self.assertEqual(len(g['face_reversed']), g['faces'])
            for phase in ('undo', 'redo'):
                if phase in value:
                    histories += 1
                    events = [e for e in value[phase]['events'] if e['name'] == phase.title()]
                    self.assertEqual(len(events), 1)
                    self.assertEqual(events[0]['objects'], value[phase]['after'])
        self.assertEqual((len(capture['results']), histories), (80, 8))

    def test_ordered_reduction_changes_success_and_source_ownership(self):
        rows = {r['id'].removeprefix('common_'): r['value'] for r in self.capture()['results']}
        result = lambda case: next(e['result'] for e in rows[case]['command']['events'] if e['name'] == 'BooleanIntersection')
        after = lambda case: rows[case]['command']['after']
        for case in ('enclosing_012', 'enclosing_201', 'double_2301', 'double_2013'):
            g = after(case)[0]
            self.assertEqual((g['name'], g['geometry_text']), ('source-0', 'geometry-1'))
            self.assertAlmostEqual(g['volume'], 1.)
        self.assertEqual(result('inside_012'), 'Success')
        self.assertEqual(result('inside_201'), 'Nothing')
        self.assertEqual(result('four_inside_0312'), 'Success')
        self.assertEqual(result('four_inside_0321'), 'Nothing')
        g = after('inside_012')[0]
        self.assertEqual((g['name'], g['geometry_text']), ('source-2', 'geometry-2'))
        self.assertAlmostEqual(g['volume'], .216)
        g = after('boundary_inside_102')[0]
        self.assertEqual((g['name'], g['geometry_text']), ('source-0', 'geometry-2'))
        g = after('shared_plane_210')[0]
        self.assertEqual((g['name'], g['geometry_text']), ('source-1', 'geometry-0'))
        g = after('duplicate_a_012')[0]
        self.assertEqual((g['name'], g['geometry_text']), ('source-1', 'geometry-2'))
        self.assertEqual(result('duplicate_a_201'), 'Failure')
        self.assertEqual(result('duplicate_b_120'), 'Failure')

    def test_capture_provenance(self):
        p = json.loads((ROOT/'docs/common-intersection-provenance.json').read_text())
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        self.assertEqual(p['settings_scheme'], 'VibocerosOracleCommonParticipationFinal_20261004')
        self.assertEqual((p['command_witnesses'], p['physical_command_records'], p['history_recipes']), (80, 80, 4))
        self.assertEqual(p['output_partition_differences'], 0)
        for path, digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(), digest, path)
