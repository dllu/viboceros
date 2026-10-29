"""Physical trim evidence and independent rejection of forged parity claims."""
from tools.verify_provenance import reconstruct_measurement
from copy import deepcopy
import json
import unittest
from .mesh_trim_cases import request, validate
from .report_mesh_trim import ROOT, native, summarize


class MeshTrimTests(unittest.TestCase):
    def test_fixed_physical_captures_and_region_semantics(self):
        _, rows, _ = native()
        self.assertEqual(len(rows), 25)
        for family in ['discovery', 'mouse', 'controls']:
            for op in request(family)['operations']:
                validate(op)
                forged = dict(op, point=[1, 1, 0])
                with self.assertRaises(ValueError):
                    validate(forged)
        for name in ['disjoint', 'partial', 'both_sides']:
            after = rows['mesh_trim_controls_'+name]['after']
            self.assertFalse(any(o['name'] == 'Source 0' for o in after))
        row = rows['mesh_trim_controls_cancel_after']
        self.assertFalse(row['success'])
        self.assertEqual(sum(o['role'] == 'output' for o in row['after']), 1)
        self.assertEqual(sum(o['role'] == 'output' for o in rows['mesh_trim_controls_two_cutters']['after']), 1)

    def test_report_rebuilds_geometry_channels_selection_status_and_history(self):
        data = json.loads((ROOT/'docs/mesh-trim-qualification.json').read_text())
        log = lambda rows, prefix: '\n'.join(prefix+' '+json.dumps(r) for r in rows)
        commands = log(data['commands'], 'MESH_TRIM_COMMAND')
        getters = log(data['getters'], 'MESH_TRIM_GETTER')
        self.assertEqual(reconstruct_measurement(data, summarize, commands, getters), data)
        self.assertEqual((data['command_matched'], data['getter_matched']), (12, 13))
        for field in ['coordinate', 'uv', 'normal', 'color', 'selection', 'ownership', 'status', 'declaration']:
            rows = deepcopy(data['commands'])
            row = next(r for r in rows if r['id'] == 'mesh_trim_controls_channels')
            obj = next(o for o in row['after']['objects'] if o['role'] == 'output')
            mesh = obj['mesh']
            if field in ['coordinate', 'uv', 'normal', 'color']:
                channel = dict(coordinate='vertices', uv='uv', normal='vertex_normals', color='colors')[field]
                mesh[channel][0][0] += 50 if field == 'color' else 1.
            elif field == 'selection':
                obj['selected'] = True
            elif field == 'ownership':
                row['sources_restored_on_undo'] = False
            elif field == 'status':
                row['success'] = False
            else:
                row['after']['channels']['matched'] = False
            with self.assertRaises(ValueError, msg=field):
                summarize(log(rows, 'MESH_TRIM_COMMAND'), getters)
        for rows in [data['getters'][:-1], data['getters']+[data['getters'][0]]]:
            with self.assertRaises(ValueError):
                summarize(commands, log(rows, 'MESH_TRIM_GETTER'))


if __name__ == '__main__':
    unittest.main()
