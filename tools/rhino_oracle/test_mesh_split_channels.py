"""Native vertex-channel parity, coherent seams, and forged-claim rejection."""
from copy import deepcopy
import json
import unittest
from .mesh_channel_comparison import compare
from .report_mesh_split_channels import ROOT, native, summarize


class MeshSplitChannelTests(unittest.TestCase):
    def test_native_channels_and_existing_ngon_policy(self):
        _, rows, _ = native()
        self.assertEqual(len(rows), 21)
        before = rows['mesh_split_channels_quad_all_post']['before'][0]['mesh']
        self.assertEqual(len(before['uv']), 4)
        self.assertEqual(len(before['vertex_normals']), 4)
        output = lambda name: [o['mesh'] for o in rows[name]['after'] if o['role'] == 'output']
        for name in ['mesh_split_channels_existing_ngon', 'mesh_split_ngon_policy_disjoint_keep_no',
                     'mesh_split_ngon_policy_partial_keep_no']:
            self.assertTrue(output(name))
            self.assertFalse(any(m['ngons'] for m in output(name)))
        row = rows['mesh_split_ngon_policy_disjoint_discard_no']
        self.assertFalse(output('mesh_split_ngon_policy_disjoint_discard_no'))
        self.assertEqual(len(row['after'][0]['mesh']['ngons']), 1)

    def test_seam_candidates_must_match_all_channels_together(self):
        a = dict(vertices=[[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]] * 2,
                 faces=[[0, 1, 2], [3, 4, 5]], uv=[[0., 0.]] * 3 + [[10., 0.]] * 3,
                 vertex_normals=[[0., 0., 1.]] * 3 + [[0., 1., 0.]] * 3,
                 colors=[[10, 20, 30, 40]] * 3 + [[50, 60, 70, 80]] * 3)
        b = deepcopy(a)
        self.assertTrue(compare(a, b)['matched'])
        b['vertex_normals'] = b['vertex_normals'][3:] + b['vertex_normals'][:3]
        self.assertFalse(compare(a, b)['matched'])

    def test_report_rebuild_and_rejection_of_changed_channels_or_claims(self):
        data = json.loads((ROOT/'docs/mesh-split-channels-qualification.json').read_text())
        log = lambda rows, prefix: '\n'.join(prefix+' '+json.dumps(r) for r in rows)
        commands = log(data['commands'], 'MESH_SPLIT_CHANNEL_COMMAND')
        getters = log(data['getters'], 'MESH_SPLIT_CHANNEL_GETTER')
        self.assertEqual(summarize(commands, getters), data)
        self.assertEqual((data['command_channels_matched'], data['getter_channels_matched']), (21, 21))
        self.assertEqual((data['command_matched'], data['getter_matched']), (18, 18))
        for field in ['uv', 'vertex_normals', 'colors', 'presence', 'count', 'ownership', 'declaration']:
            rows = deepcopy(data['commands'])
            row = next(r for r in rows if r['id'] == 'mesh_split_channels_quad_all_post')
            mesh = next(o['mesh'] for o in row['after']['objects'] if o['role'] == 'output')
            if field in ['uv', 'vertex_normals']:
                mesh[field][0][0] += 1.
            elif field == 'colors':
                mesh['colors'][0][3] = (mesh['colors'][0][3] + 50) % 256
            elif field == 'presence':
                mesh['uv'] = None
            elif field == 'count':
                mesh['uv'].pop()
            elif field == 'ownership':
                row['sources_restored_on_undo'] = False
            else:
                row['after']['channels']['matched'] = False
            with self.assertRaises(ValueError, msg=field):
                summarize(log(rows, 'MESH_SPLIT_CHANNEL_COMMAND'), getters)
        for rows in [data['commands'][:-1], data['commands']+[data['commands'][0]]]:
            with self.assertRaises(ValueError):
                summarize(log(rows, 'MESH_SPLIT_CHANNEL_COMMAND'), getters)


if __name__ == '__main__':
    unittest.main()
