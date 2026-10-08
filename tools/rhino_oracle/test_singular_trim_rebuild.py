"""Owned native Rebuild evidence for partial and holed pole boundaries."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock
from .client import OracleClient, OracleProtocolError, _validate_response
from .surface_rebuild_singular_trim_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]


class SingularTrimRebuildTests(TestCase):
    def capture(self):
        return json.loads((ROOT / 'tools/rhino_oracle/observations/surface_rebuild_singular_trim.json').read_text())

    def test_closed_private_idle_recipes(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, json.loads((ROOT / 'tools/rhino_oracle/fixtures/surface_rebuild_singular_trim.json').read_text()))
        self.assertEqual(len(q['operations']), 12)
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
            with self.assertRaisesRegex(OracleProtocolError, 'private settings scheme'):
                OracleClient().run_rhino(q, 1)
            launch.assert_not_called()

    def test_complete_capture_has_source_purity_history_and_sdk_projection_equivalence(self):
        q = self.capture()
        self.assertEqual({r['id'] for r in q['results']}, {o['id'] for o in request()['operations']})
        self.assertEqual(q['engine_version'], '8.32.26160.13001')
        for row in q['results']:
            v = row['value']
            command = v['command']
            output = command['after_script'][-1]
            self.assertTrue(command['success'])
            self.assertFalse(command['active'])
            self.assertTrue(output['valid'])
            self.assertEqual(command['after_script'][0], v['before'][0])
            self.assertEqual(v['undo']['after_script'], v['before'])
            self.assertEqual(v['redo']['after_script'], command['after_script'])
            if v['spec']['retrim']:
                self.assertEqual(output['brep'], v['sdk_trim']['project']['brep'])
                for axis in ('u', 'v'):
                    self.assertEqual(output['definition']['domain_' + axis], v['before'][0]['definition']['domain_' + axis])
            else:
                self.assertEqual(output['definition'], v['rebuild_sdk'])

    def test_caps_holes_wedges_and_both_charts_keep_collapsed_trims_without_edges(self):
        for row in self.capture()['results']:
            v = row['value']
            if not v['spec']['retrim']:
                continue
            source = v['before'][0]['brep']
            result = v['command']['after_script'][-1]['brep']
            def singular(b):
                return [t for l in b['topology']['faces'][0]['loops'] for t in l['trims'] if t['type'] == 'Singular']
            self.assertEqual(len(singular(source)), len(singular(result)), v['case'])
            self.assertGreater(len(singular(result)), 0)
            for trim in singular(result):
                self.assertIsNone(trim['edge'])
                self.assertEqual(trim['vertices'][0], trim['vertices'][1])

    def test_provenance_retains_complete_and_initial_artifacts(self):
        p = json.loads((ROOT / 'docs/singular-trim-rebuild-provenance.json').read_text())
        self.assertFalse(p['full_native_parity'])
        self.assertEqual(p['recipes'], 12)
        for name, digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / name).read_bytes()).hexdigest(), digest, name)
        initial = json.loads((ROOT / 'tools/rhino_oracle/observations/surface_rebuild_singular_trim_initial.json').read_text())
        self.assertEqual(len(initial['results']), 10)

    def test_incomplete_native_capture_is_rejected_against_its_request(self):
        initial = json.loads((ROOT / 'tools/rhino_oracle/observations/surface_rebuild_singular_trim_initial.json').read_text())
        with self.assertRaisesRegex(OracleProtocolError, 'missing=.*sphere_reversed_cap'):
            _validate_response(initial, 'rhino', request())
        _validate_response(self.capture(), 'rhino', request())

    def test_native_runner_rejects_missing_and_unexpected_ids_but_accepts_reordering(self):
        q = dict(protocol_version=1, operations=[dict(id='a', op='point'), dict(id='b', op='point')])
        def response(ids):
            return dict(protocol_version=1, engine='viboceros', iterations=1,
                        results=[dict(id=i, value=0, elapsed_ns=0) for i in ids])
        for ids in [[], ['a'], ['a', 'b', 'extra']]:
            with mock.patch.object(OracleClient, '_run_native', return_value=response(ids)):
                with self.assertRaisesRegex(OracleProtocolError, 'result ids differ'):
                    OracleClient().run_viboceros(q)
        with mock.patch.object(OracleClient, '_run_native', return_value=response(['b', 'a'])):
            self.assertEqual(len(OracleClient().run_viboceros(q)['results']), 2)

    def test_cap_projection_queries_are_derived_from_original_native_geometry(self):
        from .surface_rebuild_cap_projection_request import request as projection_request
        fixture = json.loads((ROOT / 'tools/rhino_oracle/fixtures/surface_rebuild_cap_projection.json').read_text())
        self.assertEqual(fixture, projection_request())
        capture = json.loads((ROOT / 'tools/rhino_oracle/observations/surface_rebuild_cap_projection.json').read_text())
        _validate_response(capture, 'rhino', fixture)
        self.assertEqual(sum(len(r['value']) for r in capture['results']), 66)

    def test_swapped_cap_transfer_keeps_an_isocurve_instead_of_pointwise_closest_locus(self):
        capture = json.loads((ROOT / 'tools/rhino_oracle/observations/surface_rebuild_cap_projection.json').read_text())
        projections = next(r['value'] for r in capture['results'] if r['id'] == 'sphere_swapped_cap_retrim')
        coordinates = [p['parameters'][0] for p in projections]
        self.assertGreater(max(coordinates) - min(coordinates), 4e-4)
        native = next(r['value'] for r in self.capture()['results'] if r['value']['case'] == 'sphere_swapped_cap_retrim')
        trims = native['command']['after_script'][-1]['brep']['faces'][0]['loops'][0]
        boundary = next(t for t in trims if t['iso'] == 1)
        controls = [p['point'][0] for p in boundary['definition']['control_points']]
        self.assertLess(max(controls) - min(controls), 3e-8)
        self.assertAlmostEqual(controls[0], coordinates[0], delta=1e-9)
