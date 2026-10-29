"""Empty storage comparisons must check both topology and object persistence."""
from tools.verify_provenance import reconstruct_measurement
from copy import deepcopy
import json
import unittest
from .report_mesh_boolean_commands import compare_meshes, mesh_key
from .report_mesh_boolean_empty import ROOT, native, summarize


class EmptyMeshTests(unittest.TestCase):
    def test_native_validity_selection_and_persistent_file_bytes(self):
        _, rows, exploratory, _ = native()
        self.assertEqual((len(rows), len(exploratory)), (2, 0))
        self.assertEqual([o['source'] for o in rows['boolean_empty_inside']['after']
                          if o.get('mesh') == {'vertices': [], 'faces': []}], [1])

    def test_comparison_does_not_conflate_empty_storage_with_unused_vertices(self):
        empty = mesh_key(dict(vertices=[], faces=[]))
        self.assertEqual(compare_meshes(empty, empty), (0., True))
        loose_vertex = mesh_key(dict(vertices=[[1000., 2000., 3000.]], faces=[]))
        self.assertEqual(compare_meshes(loose_vertex, empty), (None, False))

    def test_qualification_recomputes_storage_and_rejects_forged_empty_geometry(self):
        data = json.loads((ROOT/'docs/empty-mesh-qualification.json').read_text())
        log = lambda rows, prefix: '\n'.join(prefix+' '+json.dumps(r) for r in rows)
        commands = log(data['commands'], 'MESH_BOOLEAN_EMPTY_COMMAND')
        getters = log(data['getters'], 'MESH_BOOLEAN_EMPTY_GETTER')
        self.assertEqual(reconstruct_measurement(data, summarize, commands, getters), data)
        for field in ['vertices', 'name', 'removed']:
            changed = deepcopy(data['commands'])
            objects = changed[0]['after']['objects']
            empty = next(o for o in objects if o.get('mesh') == {'vertices': [], 'faces': []})
            if field == 'vertices': empty['mesh']['vertices'] = [[1000., 2000., 3000.]]
            elif field == 'name': empty['name'] = 'forged'
            else: objects.remove(empty)
            with self.assertRaises(ValueError):
                summarize(log(changed, 'MESH_BOOLEAN_EMPTY_COMMAND'), getters)


if __name__ == '__main__':
    unittest.main()
