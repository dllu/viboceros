"""Bounded face-collapse observations and content-addressed native producers."""
import copy
import gzip
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock
from .client import OracleClient, OracleProtocolError

from .generate_mesh_face_collapse import request
from .generate_mesh_face_collapse_point import request as point_request
from .generate_mesh_face_collapse_precision import request as precision_request
from .generate_mesh_face_collapse_indices import request as indices_request
from .generate_mesh_face_collapse_component_pick import request as component_request
from .generate_mesh_face_collapse_getter import request as getter_request
from .generate_mesh_face_collapse_shaded import request as shaded_request
from .generate_mesh_face_collapse_modes import request as modes_request
from .generate_mesh_face_collapse_holdouts import request as holdout_request
from .generate_mesh_face_collapse_history import request as history_request
from .mesh_face_collapse_probe import validate
from .structural_sdk import stage_helpers, validate_operations
from .mesh_face_collapse_evidence import classify

HERE = Path(__file__).resolve().parent
FAMILIES = [('mesh_face_collapse', request), ('mesh_face_collapse_point', point_request),
            ('mesh_face_collapse_precision', precision_request),
            ('mesh_face_collapse_indices', indices_request),
            ('mesh_face_collapse_component_pick', component_request),
            ('mesh_face_collapse_getter', getter_request),
            ('mesh_face_collapse_shaded', shaded_request),
            ('mesh_face_collapse_modes', modes_request),
            ('mesh_face_collapse_holdouts', holdout_request),
            ('mesh_face_collapse_history', history_request)]


class MeshFaceCollapseTests(unittest.TestCase):
    def test_success_and_acknowledged_input_do_not_prove_a_mesh_edit(self):
        root = json.loads((HERE/'observations/mesh_face_collapse_getter.json').read_text())
        self.assertEqual(len(root['results']), 4)
        for row in root['results']:
            result = classify(row['value'])
            self.assertTrue(result['command_success'])
            self.assertEqual(result['acknowledged_clicks'], 1)
            self.assertFalse(result['geometry_changed'])
            self.assertFalse(result['undo_verified'])
        native = json.loads((HERE/'observations/mesh_face_collapse.json').read_text())
        changed = classify(native['results'][0]['value'])
        self.assertTrue(changed['geometry_changed'])
        self.assertFalse(changed['undo_verified'])

    def test_complete_mesh_storage_matches_across_preselection_and_acknowledged_shaded_picks(self):
        native = json.loads((HERE/'observations/mesh_face_collapse.json').read_text())
        by_id = {r['id']: r['value'] for r in native['results']}
        picked = json.loads((HERE/'observations/mesh_face_collapse_shaded.json').read_text())
        for row in picked['results']:
            value = row['value']
            self.assertEqual(value['before'], by_id[row['id']]['before'])
            self.assertEqual(value['after'], by_id[row['id']]['after'])
            self.assertEqual(value['input_trace']['completed'], 2)
            self.assertFalse(value['input_trace']['errors'])
            result = classify(value)
            self.assertTrue(result['geometry_changed'])
            self.assertTrue(result['native_invalid_mesh_warning'])
            self.assertFalse(result['usable_reference'])

    def test_precision_capture_reports_actual_document_storage_and_both_buffers(self):
        native = json.loads((HERE/'observations/mesh_face_collapse_precision.json').read_text())
        for row in native['results']:
            after = row['value']['after']
            self.assertTrue(after['double_precision'])
            self.assertTrue(after['valid'])
            self.assertEqual(after['vertices'], after['float_vertices'])

    def test_mode_controls_preserve_native_warnings_and_do_not_admit_successful_noops(self):
        native = json.loads((HERE/'observations/mesh_face_collapse_modes.json').read_text())
        self.assertEqual(len(native['results']), 8)
        modes = {'wireframe': set(), 'shaded': set()}
        for row in native['results']:
            value = row['value']
            shaded = row['id'].endswith('-shaded')
            result = classify(value)
            self.assertEqual(result['geometry_changed'], shaded)
            self.assertEqual(result['native_invalid_mesh_warning'], shaded)
            self.assertFalse(result['undo_verified'])
            self.assertFalse(result['usable_reference'])
            self.assertTrue(value['after']['valid'])
            self.assertEqual(value['after']['vertices'], value['after']['float_vertices'])
            modes['shaded' if shaded else 'wireframe'].add(value['display_mode_id'])
        self.assertEqual(len(modes['shaded']), 1)
        self.assertEqual(len(modes['wireframe']), 1)
        self.assertFalse(modes['shaded'] & modes['wireframe'])

    def test_live_commands_require_private_homogeneous_single_iteration_batches(self):
        ordinary = request()
        repeated = dict(ordinary, iterations=2)
        boolean = dict(ordinary, iterations=True)
        mixed = copy.deepcopy(ordinary)
        other = copy.deepcopy(getter_request()['operations'][0])
        other['id'] = 'getter-fan-none'
        mixed['operations'].append(other)
        for query, scheme in [(ordinary,None), (repeated,'VibocerosOracleFaceTest'),
                              (boolean,'VibocerosOracleFaceTest'), (mixed,'VibocerosOracleFaceTest')]:
            with (self.subTest(scheme=scheme, iterations=query['iterations']),
                  mock.patch.dict(os.environ, {'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),
                  mock.patch('tools.rhino_oracle.client._rhino_process_ids', return_value=set()),
                  mock.patch('tools.rhino_oracle.client._run_logged') as launch,
                  self.assertRaisesRegex(OracleProtocolError, 'private dedicated')):
                OracleClient(launcher='/bin/true', settings_scheme=scheme).run_rhino(query)
            launch.assert_not_called()

    def test_isolated_history_controls_verify_edits_and_keep_noops_distinct(self):
        native = json.loads((HERE/'observations/mesh_face_collapse_history.json').read_text())
        digest = native['provenance']['sources']['mesh_face_collapse_probe.py']
        producer = gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes()).decode()
        self.assertLess(producer.index('doc.ClearUndoRecords(True)'), producer.index('before = value(obj.Geometry)'))
        self.assertLess(producer.index('doc.ClearRedoRecords()'), producer.index('before = value(obj.Geometry)'))
        self.assertEqual(len(native['results']), 4)
        for i, row in enumerate(native['results']):
            value = row['value']
            self.assertTrue(value['undo'])
            self.assertTrue(value['redo'])
            result = classify(value, history_isolated=True)
            self.assertEqual(result['geometry_changed'], i % 2 == 0)
            self.assertEqual(result['undo_verified'], i % 2 == 0)
            self.assertFalse(result['usable_reference'])

    def test_bounded_recipes_and_idle_helpers(self):
        for family, make in FAMILIES:
            query = make()
            validate_operations(query)
            with tempfile.TemporaryDirectory() as directory:
                stage_helpers(query, HERE, Path(directory))
                expected = {'mesh_face_collapse_probe.py', 'join_probe.py', 'merge_edges_probe.py'}
                if family in ['mesh_face_collapse_getter','mesh_face_collapse_shaded','mesh_face_collapse_modes','mesh_face_collapse_holdouts','mesh_face_collapse_history']:
                    expected.update(['conic_getter_input.py','managed_mouse_callback.py','snap_environment.py','number_token.py'])
                self.assertEqual({p.name for p in Path(directory).iterdir()}, expected)

    def test_invalid_mesh_and_face_recipes_fail_preflight(self):
        original = request()['operations'][0]
        for field, value in [('face', -1), ('face', 1000), ('channels', 'other'),
                             ('vertices', [[float('nan'), 0, 0]]*3),
                             ('faces', [[0, 1, 999]]), ('id', 'bad id'),
                             ('vertices', [['invalid',0,0]]*3), ('extra', True)]:
            op = copy.deepcopy(original)
            op[field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                validate(op)

    def test_frozen_native_recipes_and_sources_are_immutable(self):
        for family, make in FAMILIES:
            encoded = (HERE/('fixtures/'+family+'.json')).read_bytes()
            native = json.loads((HERE/('observations/'+family+'.json')).read_text())
            self.assertEqual(json.loads(encoded), make())
            self.assertEqual(native['engine'], 'rhino')
            self.assertEqual(native['provenance']['request_sha256'], hashlib.sha256(encoded).hexdigest())
            self.assertEqual([r['id'] for r in native['results']], [op['id'] for op in make()['operations']])
            for digest in native['provenance']['sources'].values():
                content = gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(content).hexdigest(), digest)
            for row in native['results']:
                self.assertEqual(row['value']['command_result'], 'Success')
                self.assertTrue(row['value']['attributes_preserved'])
                self.assertTrue(row['value']['redo'])
