"""Bounded native SubD merge recipes and immutable component identity witnesses."""
import copy
import gzip
import hashlib
import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock
from .client import OracleClient, OracleProtocolError, _compare_value
from .generate_subd_merge_faces import request
from .subd_merge_faces_probe import validate
from .structural_sdk import stage_helpers, validate_operations

HERE = Path(__file__).resolve().parent


def observations():
    return json.loads((HERE / 'observations/subd_merge_faces.json').read_text())


def subd(snapshot):
    matches = [row for row in snapshot if 'subd' in row]
    if len(matches) != 1:
        raise ValueError('one surviving SubD required')
    return matches[0]


class SubDMergeFacesTests(unittest.TestCase):
    def test_closed_bounded_recipes_and_staged_helpers(self):
        query = request()
        validate_operations(query)
        self.assertEqual(len(query['operations']), 41)
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(query, HERE, Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()}, {
                'subd_merge_faces_probe.py', 'subd_edit_scene.py', 'subd_box_probe.py',
                'hide_pt_input.py', 'number_token.py', 'join_probe.py', 'merge_edges_probe.py'})
        for op in [None, [], True, 'MergeFaces']:
            with self.assertRaises(ValueError):
                validate(op)
        for patch in [dict(id='bad _Delete'), dict(extra=True), dict(corners=[True]),
                dict(components=[dict(kind='face', index=True)]),
                dict(components=[dict(kind='edge', vertices=[0, 0])]),
                dict(components=[dict(kind='vertex', index=999)]),
                dict(components=[dict(kind='face', index=0, extra=True)]),
                dict(vertices=[[float('nan'), 0, 0]] * 4), dict(faces=[[0, 1, []]]),
                dict(creases=[[1, 0]]), dict(soft=dict(weight=50, edges=[])),
                dict(soft=dict(weight=float('nan'), edges=[[0, 1]])),
                dict(soft=dict(weight='_Delete', edges=[[0, 1]]))]:
            with self.subTest(patch=patch), self.assertRaises(ValueError):
                validate(dict(copy.deepcopy(query['operations'][0]), **patch))

    def test_capture_preflight_rejects_public_mixed_and_repeated_jobs(self):
        query = request()
        mixed = dict(query, operations=query['operations'] + [dict(op='curve_length', id='other')])
        for scheme, q in [(None, query), ('VibocerosOracleMergeTest', dict(query, iterations=True)),
                ('VibocerosOracleMergeTest', dict(query, iterations=2)),
                ('VibocerosOracleMergeTest', dict(query, operations=query['operations'] * 2)),
                ('VibocerosOracleMergeTest', mixed)]:
            with (mock.patch.dict(os.environ, {'DISPLAY': ':101', 'VIBOCEROS_ORACLE_HEADLESS': ':101'}),
                  mock.patch('tools.rhino_oracle.client._rhino_process_ids', return_value=set()),
                  mock.patch('tools.rhino_oracle.client._run_logged') as launch,
                  self.assertRaisesRegex(OracleProtocolError, 'private dedicated')):
                OracleClient(launcher='/bin/true', settings_scheme=scheme).run_rhino(q)
            launch.assert_not_called()

    def test_capture_coverage_provenance_and_component_tables(self):
        query, result = request(), observations()
        fixture = HERE / 'fixtures/subd_merge_faces.json'
        self.assertEqual(query, json.loads(fixture.read_text()))
        self.assertEqual([row['id'] for row in result['results']], [op['id'] for op in query['operations']])
        self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(), result['provenance']['request_sha256'])
        for name, digest in result['provenance']['sources'].items():
            data = (HERE / name).read_bytes()
            if hashlib.sha256(data).hexdigest() != digest:
                data = gzip.decompress((HERE / 'observations/source_snapshots' / (digest + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), digest, name)
        for row in result['results']:
            value = row['value']
            self.assertTrue(value['source_3dm'])
            self.assertTrue(value['after_3dm'])
            for phase in ['before', 'after', 'undone', 'redone']:
                data = subd(value[phase])['subd']
                self.assertTrue(data['valid'], row['id'])
                for kind, records in [('vertex', 'points'), ('edge', 'edges'), ('face', 'faces')]:
                    ids = data[kind + '_ids']
                    self.assertEqual(len(ids), len(data[records]), (row['id'], phase, kind))
                    self.assertEqual(len(ids), len(set(ids)))
                    self.assertTrue(all(i > 0 for i in ids))
                self.assertTrue(all(0 <= i < len(data['points']) for edge in data['edges'] for i in edge))
                self.assertTrue(all(0 <= i < len(data['points']) for face in data['faces'] for i in face))

    def test_complete_definition_comparison_includes_identity_and_limit_geometry(self):
        data = subd(observations()['results'][0]['value']['after'])['subd']
        for name in ['points', 'vertex_tags', 'surface_points', 'edges', 'edge_tags', 'faces',
                     'vertex_ids', 'edge_ids', 'face_ids', 'surface_mesh']:
            changed = copy.deepcopy(data)
            if name == 'surface_mesh':
                changed[name]['vertices'][0][0] += .01
            elif name.endswith('_ids') or name.endswith('_tags'):
                changed[name][0] += 1
            else:
                changed[name][0][0] += .01 if name in ['points', 'surface_points'] else 1
            differences = []
            _compare_value('subd', data, changed, 1e-7, 1e-12, differences)
            self.assertTrue(differences, name)

    def test_native_changes_preserve_attributes_and_round_trip_through_history(self):
        changed, unchanged, outcomes = 0, 0, {}
        attributes = ['source', 'name', 'layer', 'groups', 'color', 'color_source', 'grips_on']
        for row in observations()['results']:
            value = row['value']
            before, after = subd(value['before']), subd(value['after'])
            outcomes[value['command_result']] = outcomes.get(value['command_result'], 0) + 1
            for field in attributes:
                self.assertEqual(before[field], after[field], (row['id'], field))
            self.assertEqual(subd(value['undone'])['subd'], before['subd'], row['id'])
            self.assertEqual(subd(value['redone'])['subd'], after['subd'], row['id'])
            self.assertFalse(after['selected'])
            self.assertFalse(after['selected_edges'])
            if before['subd'] != after['subd']:
                changed += 1
                self.assertTrue(after['selected_faces'])
                selected_vertices = {before['subd']['vertex_ids'][i] for i in before['selected_vertices']}
                self.assertEqual({after['subd']['vertex_ids'][i] for i in after['selected_vertices']},
                                 selected_vertices.intersection(after['subd']['vertex_ids']), row['id'])
                self.assertEqual(len(value['undone']), 2, row['id'])
            else:
                unchanged += 1
                self.assertFalse(after['selected_faces'])
                self.assertFalse(after['selected_vertices'])
                # The earlier point witness is undone: no new no-op entry.
                self.assertEqual(len(value['undone']), 1, row['id'])
            self.assertEqual(len(value['redone']), 2, row['id'])
        self.assertEqual((changed, unchanged), (33, 8))
        self.assertEqual(outcomes, dict(Success=40, Cancel=1))

    def test_native_valence_cleanup_and_hard_crease_selection_difference(self):
        rows = {row['id']: row['value'] for row in observations()['results']}
        data = lambda name: subd(rows[name]['after'])['subd']
        self.assertEqual(data('tri-faces')['vertex_ids'], [1, 2, 4])
        self.assertEqual(data('tri-faces')['face_ids'], [1])
        self.assertEqual(data('strip-faces')['vertex_ids'], [1, 4, 5, 6])
        self.assertEqual(data('strip-faces')['edge_ids'], [1, 2, 6, 7])
        self.assertEqual(data('cube-faces')['vertex_ids'], [3, 4, 5, 6, 7, 8])
        self.assertEqual(data('cube-faces')['face_ids'], [2, 4, 5, 6, 1])
        self.assertEqual(len(data('hard-edge')['points']), 6)
        self.assertEqual(len(data('hard-edge')['faces'][0]), 6)
        for name in ['hard-faces', 'hard-vertex', 'soft-faces', 'soft-edge', 'soft-vertex']:
            self.assertEqual(data(name)['vertex_ids'], [1, 4, 5, 6], name)
        self.assertEqual(data('corner-survives')['vertex_tags'][0], 3)
        self.assertEqual(data('three-faces'), data('three-faces-reverse-picks'))

    def test_native_regions_with_holes_and_closed_regions_remain_unchanged(self):
        rows = {row['id']: row['value'] for row in observations()['results']}
        for name in ['single-face', 'empty-selection', 'boundary-edge', 'boundary-vertex',
                     'all-closed-faces', 'all-closed-edges', 'ring-faces', 'ring-edges']:
            value = rows[name]
            self.assertEqual(subd(value['before'])['subd'], subd(value['after'])['subd'], name)
        self.assertEqual(subd(rows['partial-ring']['after'])['subd']['face_ids'], [5, 6, 7, 8, 1])
        for name in ['disjoint-face-pairs', 'disjoint-edge-pairs']:
            self.assertEqual(subd(rows[name]['after'])['subd']['face_ids'], [1, 3])
        self.assertEqual(subd(rows['mixed-face-edge']['after'])['selected_faces'], [1, 2])
        self.assertEqual(subd(rows['mixed-vertex-face']['after'])['selected_faces'], [3, 1])
