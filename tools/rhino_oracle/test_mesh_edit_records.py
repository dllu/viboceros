"""Bounded native mesh edits, unchanged records, recovery and private launch."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .mesh_edit_records_probe import CASES, definition, request, run, validate

ROOT = Path(__file__).resolve().parents[2]


class MeshEditRecordTests(TestCase):
    def test_closed_recipes_and_owned_document(self):
        fixture = json.loads((ROOT / 'tools/rhino_oracle/fixtures/mesh_edit_records.json').read_text())
        self.assertEqual(request(), fixture)
        for change in (dict(extra=1), dict(id='x\n_Delete'), dict(id='λ'),
                       dict(id=[]), dict(case='anything'), dict(case=[]), dict(op='mesh')):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate(dict(fixture['operations'][0], **change))
        rhino = mock.Mock()
        rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError, 'empty owned document'):
            run(fixture['operations'][0], {'Rhino': rhino, 'System': mock.Mock()})

    def test_private_launch_guards(self):
        for scheme, count, display, headless in ((None, 1, ':301', ':301'),
                ('VibocerosOracleMeshRecords', True, ':301', ':301'),
                ('VibocerosOracleMeshRecords', 2, ':301', ':301'),
                ('VibocerosOracleMeshRecords', 1, ':0', None),
                ('VibocerosOracleMeshRecords', 1, ':301', ':302')):
            q = request()
            q['iterations'] = count
            env = {'DISPLAY': display}
            if headless:
                env['VIBOCEROS_ORACLE_HEADLESS'] = headless
            with mock.patch.dict(os.environ, env, clear=True), mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError, OracleProtocolError)):
                    OracleClient(settings_scheme=scheme).run_rhino(q, 1)
                launch.assert_not_called()

    def test_native_face_records_colors_topology_and_recovery(self):
        r = json.loads((ROOT / 'tools/rhino_oracle/observations/mesh_edit_records.json').read_text())
        self.assertEqual(r['engine_version'], '8.32.26160.13001')
        self.assertEqual(len(r['results']), len(CASES))
        for case, row in zip(CASES, r['results']):
            with self.subTest(case=case):
                self.assertEqual(row['id'], 'mesh-records-'+case)
                points, faces, moves = definition(case)
                v = row['value']
                self.assertEqual(v['before']['mesh'], dict(vertices=points, faces=faces))
                self.assertEqual(v['restored'], v['before'])
                self.assertEqual(v['edited']['mesh']['faces'], faces)
                edited = [moves.get(i, p) for i, p in enumerate(points)]
                self.assertEqual(v['edited']['mesh']['vertices'], edited)
                self.assertEqual(v['edited']['colors'], v['before']['colors'])
                self.assertFalse(v['edited']['closed'])
                self.assertEqual(v['edited']['valid'], case in ('triangle_line', 'quad_line'))
                self.assertIsNotNone(v['edited']['closest'])
                if case in ('triangle_point', 'quad_point'):
                    self.assertEqual(v['edited']['edges'], [])
                    self.assertEqual(v['edited']['face_normals'], [[0., 0., 0.]])
                    self.assertEqual(v['edited']['closest'], dict(face=0, point=[0., 0., 0.]))
                if case in ('triangle_point', 'quad_point', 'triangle_line', 'quad_line'):
                    self.assertEqual(v['edited']['mass'], dict(area=0., centroid=[0., 0., 0.]))
        mixed = r['results'][0]['value']['edited']
        self.assertEqual(len(mixed['edges']), 4)
        self.assertEqual([e['faces'] for e in mixed['edges'] if len(e['faces']) > 1], [[0, 1, 1]])
        self.assertEqual(mixed['face_normals'], [[0., 0., -1.], [0., 0., 0.]])
        self.assertEqual(mixed['mass'], dict(area=8., centroid=[0., 0., 0.]))

    def test_provenance(self):
        p = json.loads((ROOT / 'docs/mesh-edit-records-provenance.json').read_text())
        self.assertTrue(p['private_xvfb'])
        self.assertEqual(p['native_recipes'], len(CASES))
        self.assertFalse(p['full_native_parity'])
        for path, expected in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected, path)
