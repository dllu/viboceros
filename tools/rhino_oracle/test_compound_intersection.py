"""Closed oriented-shell recipes and private-display instrumentation guards."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .compound_intersection_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]


class CompoundIntersectionTests(TestCase):
    def capture(self):
        return json.loads((ROOT/'tools/rhino_oracle/observations/compound_intersection.json').read_text())

    def test_closed_requests_and_owned_document_launch(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/compound_intersection.json').read_text()))
        for change in (dict(id='bad\n_Exit'), dict(id=[]), dict(case=[]), dict(case='_Exit'),
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

    def test_capture_retains_events_signed_sdk_results_and_failure_markers(self):
        r = self.capture()
        self.assertEqual(r['engine_version'], '8.32.26160.13001')
        self.assertEqual([o['id'] for o in r['results']], [o['id'] for o in request()['operations']])
        sdk, commands, histories = [], [], 0
        for row in r['results']:
            v = row['value']
            if row['id'].startswith('compound_sdk_'):
                sdk.append(row)
                self.assertEqual(len(v['inputs']), 2)
                self.assertTrue(all(g['valid'] and g['solid'] for g in v['inputs']))
                continue
            commands.append(row)
            c = v['command']
            events = [e for e in c['events'] if e['name'] == 'BooleanIntersection']
            self.assertEqual(len(events), 1, row['id'])
            self.assertIn(events[0]['result'], ('Success', 'Failure', 'Nothing'))
            self.assertEqual(events[0]['objects'], c['after'])
            self.assertNotIn('Unknown command:', c['history'])
            self.assertTrue(all(g['valid'] and g['solid'] for g in v['before']))
            for phase in ('undo', 'redo'):
                if phase in v:
                    histories += 1
                    h = v[phase]
                    ends = [e for e in h['events'] if e['name'] == phase.title()]
                    self.assertEqual(len(ends), 1)
                    self.assertEqual(ends[0]['objects'], h['after'])
        self.assertEqual((len(sdk), len(commands), histories), (9, 32, 8))
        by_id = {r['id'].removeprefix('compound_'): r['value'] for r in r['results']}
        self.assertTrue(by_id['sdk_inner_equal_inner']['returned_null'])
        self.assertFalse(by_id['sdk_inner_unopened']['returned_null'])
        self.assertEqual(by_id['sdk_inner_unopened']['outputs'], [])
        inward = by_id['sdk_inner_other']['outputs'][0]
        self.assertEqual(inward['orientation'], 'Inward')
        self.assertAlmostEqual(inward['volume'], -1.875)
        failure = by_id['second_extra_disjoint_cross']
        self.assertFalse(failure['sdk_unions']['second']['outputs'][0]['solid'])
        markers = [g for g in failure['command']['after'] if g['kind'] == 'TextDot']
        self.assertEqual(len(markers), 2)
        self.assertTrue(all(g['text'] == '!' for g in markers))

    def test_set_union_and_common_policies_remain_distinct(self):
        rows = {r['id'].removeprefix('compound_'): r['value'] for r in self.capture()['results']}
        for case, volumes in (
            ('first_contains_inner', [2.25, 3.75]),
            ('second_contains_inner', [5.0625]),
            ('common_contains_inner', [.3125]),
            ('first_inside_inner', [2.25, 3.75, .03125]),
            ('second_disjoint', [2.25, 3.75, 1.]),
            ('island_second_contains', [6.8125, 1.421875, 1.75, .640625]),
        ):
            after = rows[case]['command']['after']
            self.assertEqual(len(after), len(volumes), case)
            for g, volume in zip(after, volumes):
                self.assertAlmostEqual(g['volume'], volume, places=10, msg=case)
                self.assertEqual(g['orientation'], 'Outward')
        for side in ('first', 'second'):
            self.assertTrue(all(g['selected'] for g in rows[side+'_contains_inner_pre']['command']['after']))
        self.assertEqual(rows['first_inside_inner']['command']['after'][-1]['geometry_text'], 'geometry-2')
        self.assertEqual(rows['common_contains_inner']['command']['after'][0]['geometry_text'], 'geometry-2')

    def test_capture_provenance(self):
        p = json.loads((ROOT/'docs/compound-intersection-provenance.json').read_text())
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['sdk_pair_witnesses'], p['command_witnesses']), (9, 32))
        self.assertEqual((p['physical_command_records'], p['source_preservation_failure_records']), (31, 1))
        self.assertEqual((p['previous_capture_physical_replays'], p['previous_capture_singular_rejections']), (64, 1))
        self.assertEqual(p['output_partition_differences'], 1)
        partitions = json.loads((ROOT/'docs/compound-intersection-partitions.json').read_text())
        self.assertEqual(len(partitions), 1)
        self.assertEqual(partitions[0]['case'], 'first_extra_cross')
        for path, digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(), digest, path)
