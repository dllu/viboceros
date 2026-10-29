"""Audit immutable split workflows and reject forged surface/topology claims."""
from tools.verify_provenance import reconstruct_measurement
from copy import deepcopy
import json
import unittest
from .report_mesh_split import ROOT, native, summarize


class MeshSplitTests(unittest.TestCase):
    def test_fixed_captures_and_shared_preferences(self):
        _, rows, _ = native()
        self.assertEqual(len(rows), 65)
        for name, token in [('after_self_zero', 'Tolerance=Zero'),
                            ('intersect_after_document', 'Tolerance=Document'),
                            ('intersect_after_drop', 'CoplanarFaces=Drop')]:
            self.assertIn(token, rows['mesh_split_policy_'+name]['history'])
        self.assertIn('CoplanarFaces=Polylines', rows['mesh_split_selection_intersect_after_keep']['history'])
        value = rows['mesh_split_policy_cross_inputs']
        self.assertFalse(value['success'])
        self.assertFalse(any(o['selected'] for o in value['after']))

    def test_report_rebuilds_geometry_topology_selection_and_history(self):
        data = json.loads((ROOT/'docs/mesh-split-qualification.json').read_text())
        log = lambda rows, prefix: '\n'.join(prefix+' '+json.dumps(r) for r in rows)
        commands = log(data['commands'], 'MESH_SPLIT_COMMAND')
        getters = log(data['getters'], 'MESH_SPLIT_GETTER')
        self.assertEqual(reconstruct_measurement(data, summarize, commands, getters), data)
        self.assertEqual((data['command_matched'], data['getter_matched']), (46, 53))
        for field in ['coordinate', 'selection', 'ownership', 'status', 'ngon_boundary',
                      'ngon_coverage', 'ngon_declaration']:
            rows = deepcopy(data['commands'])
            row = next(r for r in rows if r['id'] == 'mesh_split_controls_box_ngons_yes')
            obj = next(o for o in row['after']['objects'] if o['role'] == 'output')
            if field == 'coordinate':
                obj['mesh']['vertices'][0][0] += 1.
            elif field == 'selection':
                obj['selected'] = True
            elif field == 'ownership':
                row['sources_restored_on_undo'] = False
            elif field == 'status':
                row['success'] = False
            elif field == 'ngon_boundary':
                obj['mesh']['ngons'][0]['vertices'].pop()
            elif field == 'ngon_coverage':
                obj['mesh']['ngons'][0]['faces'].pop()
            else:
                row['after']['ngons'] = False
            with self.assertRaises(ValueError, msg=field):
                summarize(log(rows, 'MESH_SPLIT_COMMAND'), getters)
        for rows in [data['commands'][:-1], data['commands']+[data['commands'][0]]]:
            with self.assertRaises(ValueError):
                summarize(log(rows, 'MESH_SPLIT_COMMAND'), getters)


if __name__ == '__main__':
    unittest.main()
