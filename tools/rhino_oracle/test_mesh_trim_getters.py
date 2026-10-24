"""Independent native getter evidence and owned callback cleanup checks."""
from copy import deepcopy
import json
import unittest
from unittest.mock import Mock, patch
from .mesh_trim_pick_trace import PickTrace, observing_picks
from .report_mesh_trim_getters import ROOT, native, summarize, verify_sdk, verify_trace


class MeshTrimGetterTests(unittest.TestCase):
    def test_native_preselection_physical_inputs_and_sdk_partitions(self):
        _, rows, _ = native()
        self.assertEqual(len(rows), 29)
        for family in ['getter_controls', 'verified_getters']:
            for shape in ['sheet_pre', 'box_pre']:
                value = rows['mesh_trim_'+family+'_'+shape]
                self.assertNotIn('_Pause', value['input_script'])
                self.assertTrue(value['success'])
                self.assertTrue(any(o['role'] == 'output' for o in value['after']))
                self.assertFalse(any(o['selected'] for o in value['after']))
        value = rows['mesh_trim_getter_controls_two_meshes_sdk']
        control = verify_sdk(value)
        self.assertAlmostEqual(control['union_area'], 8.)
        self.assertAlmostEqual(control['command_retained_area'], 8.)
        self.assertAlmostEqual(control['command_surface_deviation'], 0.)
        self.assertEqual(len(control['part_areas']), 4)
        for suffix in ['mesh_only_sdk', 'horizontal_only_sdk']:
            control = verify_sdk(rows['mesh_trim_getter_controls_'+suffix])
            self.assertEqual(control['part_areas'], [4., 4.])
            self.assertAlmostEqual(control['command_retained_area'], 4.)

    def test_native_click_and_sdk_controls_reject_changed_evidence(self):
        _, rows, _ = native()
        original = rows['mesh_trim_pick_controls_curve_direct_quad']
        for field in ['pixel', 'viewport', 'prompt', 'ray', 'point', 'source']:
            value = deepcopy(original)
            expected = value['trim_pick_trace']['expected'][0]
            actual = value['trim_pick_trace']['actual'][0]
            if field == 'pixel': actual['pixel'][0] += 10
            elif field == 'viewport': actual['viewport'] = 'another viewport'
            elif field == 'prompt': actual['prompt'] = 'Command'
            elif field == 'ray': expected['ray'][1] = expected['ray'][0]
            elif field == 'point': expected['hits'][0]['point'][0] += 1.
            else: expected['hits'] = [h for h in expected['hits'] if h['source'] != 2]
            with self.assertRaises(ValueError, msg=field): verify_trace(value)
        for field in ['ownership', 'area', 'coverage']:
            value = deepcopy(rows['mesh_trim_getter_controls_two_meshes_sdk'])
            sdk = value['trim_split_control'][0]
            if field == 'ownership': sdk['inputs_unchanged'] = False
            elif field == 'area': sdk['parts'].pop()
            else:
                for part in sdk['parts']:
                    for point in part['vertices']: point[1] += 1.
            with self.assertRaises(ValueError, msg=field): verify_sdk(value)

    def test_report_rebuild_and_rejection_of_changed_geometry_or_status(self):
        data = json.loads((ROOT/'docs/mesh-trim-getter-qualification.json').read_text())
        def log(rows, suffix):
            lines = []
            for row in rows:
                family = next(f for f in ['getter_controls', 'verified_getters', 'plane_getters']
                              if row['id'].startswith('mesh_trim_'+f+'_'))
                lines.append('MESH_TRIM_'+family.upper()+'_'+suffix+' '+json.dumps(row))
            return '\n'.join(lines)
        commands, getters = log(data['commands'], 'COMMAND'), log(data['getters'], 'GETTER')
        self.assertEqual(summarize(commands, getters), data)
        self.assertEqual((data['command_matched'], data['getter_matched']), (6, 6))
        for field in ['geometry', 'selection', 'storage', 'status']:
            rows = deepcopy(data['commands'])
            row = next(r for r in rows if r['id'] == 'mesh_trim_verified_getters_sheet_pre')
            output = next(o for o in row['after']['objects'] if o['role'] == 'output')
            if field == 'geometry': output['mesh']['vertices'][0][0] += 1.
            elif field == 'selection': output['selected'] = True
            elif field == 'storage': row['sources_restored_on_undo'] = False
            else: row['success'] = False
            with self.assertRaises(ValueError, msg=field): summarize(log(rows, 'COMMAND'), getters)
        with self.assertRaises(ValueError): summarize(commands, log(data['getters'][:-1], 'GETTER'))


class PickTraceCleanupTests(unittest.TestCase):
    def test_callback_scope_closes_on_input_and_registration_failure(self):
        for registration_fails in [False, True]:
            trace = Mock()
            if registration_fails: trace.start.side_effect = ValueError('registration failed')
            with patch(__package__+'.mesh_trim_pick_trace.PickTrace', return_value=trace):
                with self.assertRaises(ValueError):
                    with observing_picks({}, None, 1, True):
                        raise ValueError('input failed')
            trace.close.assert_called_once_with()

    def test_both_callbacks_close_even_when_one_disable_fails(self):
        released = []
        class Callback:
            def __init__(self, name, fail): self.name, self.fail = name, fail
            @property
            def Enabled(self): return True
            @Enabled.setter
            def Enabled(self, value):
                released.append((self.name, value))
                if self.fail: raise ValueError('disable failed')
        trace = PickTrace.__new__(PickTrace)
        trace.callback, trace.motion_callback = Callback('click', True), Callback('motion', False)
        with self.assertRaises(ValueError): trace.close()
        self.assertEqual(released, [('click', False), ('motion', False)])


if __name__ == '__main__':
    unittest.main()
