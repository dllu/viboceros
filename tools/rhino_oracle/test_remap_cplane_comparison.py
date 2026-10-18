"""Geometry mutations, history identity and errors must survive projection."""
import copy
import json
from pathlib import Path
from unittest import TestCase
from unittest.mock import patch

from .audit import compare_audit_response
from .client import OracleClient, OracleProtocolError, compare_responses
from .remap_cplane_comparison import COMMON, STATES, canonical
from .remap_cplane_probe import validate_capture


def records(suffix=''):
    root = Path(__file__).parent
    q = json.loads((root / ('fixtures/remap_cplane'+suffix+'.json')).read_text())
    native = json.loads((root / ('observations/remap_cplane'+suffix+'.json')).read_text())
    local = dict(protocol_version=1, engine='viboceros', engine_version='comparison-test', iterations=1, results=[])
    # Synthetic local values test the comparator; real Rust replay is separate.
    for row in native['results']:
        v = row['value']
        value = {key: copy.deepcopy(v[key]) for key in COMMON}
        value['target_rejections'] = v['history'].count('Specify a different CPlane.')
        if 'setup' in v:
            value['setup'] = {key: copy.deepcopy(v['setup'][key]) for key in ('success', 'after')}
        local['results'].append(dict(id=row['id'], value=value, elapsed_ns=100))
    return q, local, native


def subset(q, local, native, index):
    return (dict(q, operations=[q['operations'][index]]),
            dict(local, results=[local['results'][index]]),
            dict(native, results=[native['results'][index]]))


class RemapComparisonTests(TestCase):
    def test_live_capture_rejects_macros_that_would_leave_the_name_getter_pending(self):
        q, _, _ = records()
        for op in q['operations']: validate_capture(op)
        bad = copy.deepcopy(q['operations'][36]); bad.pop('retry_target')
        with self.assertRaises(ValueError): validate_capture(bad)
        bad['retry_target'] = 'Top'
        with self.assertRaises(ValueError): validate_capture(bad)
        bad['target'] = 'Front'; bad['retry_target'] = 'Right'
        with self.assertRaises(ValueError): validate_capture(bad)

    def test_explicit_request_compares_all_forty_records_without_mutating_inputs(self):
        for suffix in ('', '_copy'):
            q, local, native = records(suffix)
            before = copy.deepcopy((q, local, native))
            self.assertTrue(compare_responses(local, native, 1e-9, 0., request=q).passed)
            self.assertEqual((q, local, native), before)
            self.assertFalse(compare_responses(local, native, 1e-9, 0.).passed)
        q, local, native = records()
        q['operations'][0]['op'] = 'point_distance'
        self.assertFalse(compare_responses(local, native, request=q).passed)

    def test_every_state_keeps_coordinates_selection_identity_groups_names_and_planes(self):
        q, local, native = subset(*records(), 0)
        def change_point(s): s['objects'][0]['points'][0][0] += 1e-4
        def change_selection(s): s['objects'][0]['selected'] = not s['objects'][0]['selected']
        def change_identity(s): s['objects'][0]['source'] = None
        def change_membership(s): s['objects'][0]['groups'] = []
        def change_count(s): s['groups'][0]['count'] += 1
        def change_name(s): s['objects'][0]['name'] += ' changed'
        def change_plane(s): s['plane']['origin'][0] += 1e-4
        for state in STATES:
            for mutation in (change_point, change_selection, change_identity, change_membership, change_count, change_name, change_plane):
                changed = copy.deepcopy(local)
                mutation(changed['results'][0]['value'][state])
                with self.subTest(state=state, mutation=mutation.__name__):
                    report = compare_responses(changed, native, 1e-9, 0., request=q)
                    self.assertFalse(report.passed)
                    self.assertTrue(any('.'+state+'.' in p for p in report.operations[0].differences))

    def test_curve_surface_mesh_and_grip_fields_are_not_omitted(self):
        cases = [
            (14, ('curve', 'control_points', 0, 'weight')),
            (14, ('curve', 'knots', 3)),
            (18, ('surface', 'control_points', 4, 'point', 2)),
            (18, ('surface', 'knots_v', 3)),
            (22, ('mesh', 'vertices', 0, 2)),
            (22, ('mesh', 'faces', 0, 1)),
            (30, ('grips', 0, 'point', 1)),
            (32, ('grips', 0, 'selected')),
        ]
        for index, path in cases:
            q, local, native = subset(*records(), index)
            value = local['results'][0]['value']['after']['objects'][0]
            for part in path[:-1]: value = value[part]
            value[path[-1]] = not value[path[-1]] if type(value[path[-1]]) is bool else value[path[-1]] + 1
            self.assertFalse(compare_responses(local, native, 1e-9, 0., request=q).passed, path)

    def test_group_numbers_may_vary_but_identity_changes_across_undo_are_visible(self):
        q, local, native = subset(*records(), 1)
        for state in STATES:
            for group in local['results'][0]['value'][state]['groups']:
                if group['name'] != 'Owned remap group': group['name'] = 'Group900'
        self.assertTrue(compare_responses(local, native, request=q).passed)
        local['results'][0]['value']['undo']['groups'][1]['name'] = 'Group901'
        self.assertFalse(compare_responses(local, native, request=q).passed)

    def test_end_command_contradictions_unknown_metadata_and_malformed_groups_are_rejected(self):
        q, local, native = subset(*records(), 1)
        cases = []
        for key, value in [('success', False), ('events', []), ('history', None), ('unrecognized', 1)]:
            changed = copy.deepcopy(native); changed['results'][0]['value'][key] = value; cases.append(changed)
        changed = copy.deepcopy(native)
        changed['results'][0]['value']['events'][-1]['objects']['objects'][0]['name'] = 'contradiction'
        cases.append(changed)
        for changed in cases:
            with self.assertRaises(OracleProtocolError): compare_responses(local, changed, request=q)
        for name in ('', 'Arbitrary name', None):
            changed = copy.deepcopy(local); changed['results'][0]['value']['undo']['groups'][1]['name'] = name
            with self.assertRaises(OracleProtocolError): compare_responses(changed, native, request=q)

    def test_copy_setup_and_rejection_and_undo_flags_remain_compared(self):
        q, local, native = records('_copy')
        local['results'][0]['value']['setup']['success'] = True
        self.assertFalse(compare_responses(local, native, request=q).passed)
        for key in ('success', 'undo_returned', 'redo_returned', 'target_rejections'):
            q, local, native = subset(*records(), 36)
            value = local['results'][0]['value']
            value[key] = not value[key] if type(value[key]) is bool else value[key] + 1
            self.assertFalse(compare_responses(local, native, request=q).passed, key)

    def test_api_replay_passes_the_contract_and_keeps_command_failures_as_errors(self):
        q, local, native = records()
        audit = {key: value for key, value in local.items() if key != 'results'}
        audit['outcomes'] = [dict(status='success', result=row) for row in local['results']]
        audit['outcomes'][0] = dict(status='failure', id=local['results'][0]['id'], error=dict(kind='command', message='rejected target'))
        report = compare_audit_response(audit, native, 1e-9, 0., request=q)
        self.assertEqual((report.matched, report.native_failed, report.passed), (37, 1, False))
        self.assertIsNone(report.operations[0].comparison)
        with patch.object(OracleClient, 'run_viboceros_audit', return_value=audit):
            self.assertEqual(OracleClient().replay(q, native, 1e-9, 0.).matched, 37)
        q, local, native = records('_copy')
        with self.assertRaises(OracleProtocolError): canonical(q['operations'][0], local['results'][0]['value'] | {'setup': None}, native=False)
