"""Frozen allocating command workflows and completed-command selection history."""
import base64
import copy
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from .client import OracleClient, OracleProtocolError
from .generate_subd_merge_allocation import request as allocation
from .generate_subd_merge_allocation_keyboard import request as keyboard
from .generate_subd_result_history import request as history
from .qualification_sources import SUBD_EDIT_HISTORY_MODULES, with_shared_sources
from .structural_sdk import stage_helpers, validate_operations
from .subd_merge_allocation_cases import CASES, validate as validate_allocation
from .subd_result_history_probe import validate as validate_history
from .test_merge_faces_command import frozen

HERE = Path(__file__).resolve().parent
FAMILIES = [("subd_merge_allocation", allocation, 16),
            ("subd_merge_allocation_finish", allocation, 16),
            ("subd_merge_allocation_live", allocation, 16),
            ("subd_merge_allocation_keyboard", keyboard, 4),
            ("subd_result_history", history, 8),
            ("subd_result_history_reselect", history, 8)]


def record(family):
    return json.loads((HERE / 'observations' / (family + '.json')).read_text())


def owner(rows):
    return next(r for r in rows if 'subd' in r)


def definition(s):
    # Native linked-list order can change on Duplicate/3DM readback. Compare
    # complete control records by persistent ID, including oriented face loops.
    ids = s['vertex_ids']
    return dict(vertices=sorted((i, p, t) for i, p, t in zip(ids, s['points'], s['vertex_tags'])),
                edges=sorted((i, [ids[j] for j in e], t)
                             for i, e, t in zip(s['edge_ids'], s['edges'], s['edge_tags'])),
                faces=sorted((i, [ids[j] for j in f], c)
                             for i, f, c in zip(s['face_ids'], s['faces'], s['face_colors'])),
                valid=s['valid'])


class SubDEditHistoryTests(unittest.TestCase):
    def test_fixed_requests_archives_events_and_all_frozen_sources(self):
        archives = workflows = 0
        for family, make, count in FAMILIES:
            query = make()
            validate_operations(query)
            path = HERE / 'fixtures' / (family + '.json')
            self.assertEqual(json.loads(path.read_text()), query)
            r = record(family)
            self.assertEqual(len(r['results']), count)
            workflows += count
            self.assertEqual([o['id'] for o in query['operations']], [s['id'] for s in r['results']])
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), r['provenance']['request_sha256'])
            for name in r['provenance']['sources']:
                frozen(r, name)
            for row in r['results']:
                v = row['value']
                for name in ['source_3dm', 'after_3dm'] + (['merged_3dm'] if 'sdk_copy' in v else []):
                    self.assertTrue(base64.b64decode(v[name], validate=True).startswith(b'3D Geometry File Format'))
                    archives += 1
                if 'events' in v:
                    self.assertEqual(v['events'], [dict(command='MergeFaces', result='Success'),
                                                  dict(command='SubDivide', result='Success')])
                    self.assertEqual(len(v['undone']), 2)
                    self.assertEqual(len(v['redone']), 2)
        self.assertEqual((workflows, archives), (68, 188))

    def test_recipes_are_bound_to_frozen_producer_bodies(self):
        self.assertEqual(json.loads((HERE / 'fixtures/subd_merge_allocation_recipes.json').read_text()), CASES)
        for family, _, _ in FAMILIES:
            r = record(family)
            self.assertEqual(frozen(r, 'subd_merge_allocation_cases.py'),
                             (HERE / 'subd_merge_allocation_cases.py').read_bytes())
            self.assertEqual(frozen(r, 'merge_faces_command_cases.py'),
                             (HERE / 'merge_faces_command_cases.py').read_bytes())

    def test_public_duplicate_and_archive_copies_allocate_matching_ids(self):
        for family, _, _ in FAMILIES[:4]:
            for row in record(family)['results']:
                v = row['value']
                merged = owner(v['merged'])['subd']
                for key in ['sdk_copy', 'sdk_archive']:
                    self.assertEqual(definition(v[key]['before']), definition(merged), (family, row['id'], key))
                self.assertEqual(definition(v['sdk_copy']['after']), definition(v['sdk_archive']['after']))
                for phase in ['before', 'after']:
                    s = v['sdk_copy'][phase]
                    for key in ['vertex_ids', 'edge_ids', 'face_ids']:
                        self.assertEqual(len(s[key]), len(set(s[key])))
                if v['local_face'] is None:
                    self.assertEqual(definition(owner(v['after'])['subd']), definition(v['sdk_copy']['after']))

    def test_command_callback_children_are_cleared_before_script_and_keyboard_return(self):
        local_callbacks = 0
        for family in ['subd_merge_allocation_live', 'subd_merge_allocation_keyboard']:
            for row in record(family)['results']:
                v = row['value']
                if family.endswith('_keyboard'):
                    self.assertEqual(v['transport'], 'RhinoApp.SendKeystrokes')
                self.assertEqual(owner(v['after'])['selected_faces'], [])
                if owner(v['after_endcommand'])['selected_faces']:
                    local_callbacks += 1
                    self.assertIsNotNone(v['local_face'])
                    self.assertNotEqual(owner(v['after_endcommand'])['selected_faces'],
                                        owner(v['after'])['selected_faces'])
                for phase in v['undone'] + v['redone']:
                    self.assertEqual(owner(phase)['selected_faces'], [])
        self.assertGreater(local_callbacks, 6)

    def test_redo_only_restores_automatic_faces_that_have_never_been_deselected(self):
        for family in ['subd_result_history', 'subd_result_history_reselect']:
            for row in record(family)['results']:
                name, v = row['id'], row['value']
                self.assertEqual(owner(v['after_command'])['selected_faces'], [1])
                self.assertEqual(owner(v['undone'])['selected_faces'], [])
                self.assertEqual(owner(v['redone'])['selected_faces'], [1] if name == 'keep' else [])
                self.assertFalse(owner(v['undone'])['selected'])
                self.assertEqual(owner(v['redone'])['selected'], name == 'whole')
                if family.endswith('_reselect'):
                    self.assertEqual(owner(v['reselected'])['selected_faces'], [1])
                    self.assertEqual(owner(v['reselected_redo'])['selected_faces'], [])
                    self.assertFalse(owner(v['reselected_redo'])['selected'])

    def test_closed_validators_and_staged_dependency_inventory(self):
        for make, validate, helper, cases in [
                (allocation, validate_allocation, 'subd_merge_allocation_probe.py', 'subd_merge_allocation_cases.py'),
                (history, validate_history, 'subd_result_history_probe.py', 'subd_merge_allocation_cases.py')]:
            query = make()
            for op in [None, [], True, {}, dict(query['operations'][0], extra=True),
                       dict(query['operations'][0], case=[]), dict(query['operations'][0], id='_Delete')]:
                with self.assertRaises(ValueError):
                    validate(op)
            with tempfile.TemporaryDirectory() as directory:
                stage_helpers(query, HERE, Path(directory))
                self.assertEqual({p.name for p in Path(directory).iterdir()}, {
                    helper, cases, 'subd_state.py', 'merge_faces_command_cases.py',
                    'merge_faces_command_input.py', 'subd_edit_scene.py', 'subd_box_probe.py',
                    'number_token.py', 'join_probe.py', 'merge_edges_probe.py'})

    def test_public_mixed_repeated_and_oversized_captures_are_rejected_before_launch(self):
        for make in [allocation, history]:
            q = make()
            for scheme, query in [(None, q), ('VibocerosOracleHistoryTest', dict(q, iterations=True)),
                    ('VibocerosOracleHistoryTest', dict(q, iterations=2)),
                    ('VibocerosOracleHistoryTest', dict(q, operations=q['operations'] * 9)),
                    ('VibocerosOracleHistoryTest', dict(q, operations=q['operations'] + [dict(op='curve_length', id='other')]))]:
                with (mock.patch.dict(os.environ, {'DISPLAY': ':101', 'VIBOCEROS_ORACLE_HEADLESS': ':101'}),
                      mock.patch('tools.rhino_oracle.client._rhino_process_ids', return_value=set()),
                      mock.patch('tools.rhino_oracle.client._run_logged') as launch,
                      self.assertRaisesRegex(OracleProtocolError, 'private dedicated')):
                    OracleClient(launcher='/bin/true', settings_scheme=scheme).run_rhino(copy.deepcopy(query))
                launch.assert_not_called()

    def test_current_receipt_binds_shared_implementation_and_native_inputs(self):
        root = HERE.parent.parent
        receipt = json.loads((root / 'docs/geometry/subd-edit-history-validation.json').read_text())
        self.assertEqual((receipt['allocation_workflows'], receipt['selection_workflows']), (52, 16))
        self.assertEqual(receipt['completed_command_selection_workflows'], 20)
        self.assertEqual(receipt['coordinate_epsilon'], 1e-7)
        self.assertTrue(set(SUBD_EDIT_HISTORY_MODULES) <= set(receipt['sources']))
        for path in SUBD_EDIT_HISTORY_MODULES:
            self.assertTrue(set(SUBD_EDIT_HISTORY_MODULES) <= set(with_shared_sources([path])), path)
        for name, digest in receipt['sources'].items():
            self.assertEqual(hashlib.sha256((root / name).read_bytes()).hexdigest(), digest, name)
        for family, hashes in receipt['families'].items():
            for folder, digest in hashes.items():
                self.assertEqual(hashlib.sha256((HERE / folder / (family + '.json')).read_bytes()).hexdigest(), digest)
