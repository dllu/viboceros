"""Original cutter lifetime evidence and rejection of changed continuation claims."""
from copy import deepcopy
import json
import unittest
from .mesh_boolean_comparison import measures
from .report_mesh_trim_continuations import ROOT, native, summarize


class MeshTrimContinuationTests(unittest.TestCase):
    def test_original_cutter_still_cuts_after_replacement_and_deletion(self):
        _, rows, _ = native()
        self.assertEqual(len(rows), 10)
        for suffix in ['replaced_cutter', 'deleted_cutter']:
            value = rows['mesh_trim_cutter_policy_'+suffix]
            target = next(o for o in value['after'] if o['name'] == 'Source 2')
            # Without the removed portion of Source 0, the other cutter alone
            # leaves area 0.8. Native retains the lower-left region, area 1.2.
            self.assertAlmostEqual(measures([target['mesh']])[0], 1.2)
            self.assertFalse(any(o['role'] == 'source' and o['source'] == 0 for o in value['after']))
        value = rows['mesh_trim_continuation_shared_cancel_after_two']
        self.assertFalse(value['success'])
        self.assertFalse(any(o['name'] == 'Source 0' for o in value['after']))

    def test_report_rebuild_and_rejection_of_changed_surfaces_status_and_history(self):
        data = json.loads((ROOT/'docs/mesh-trim-continuation-qualification.json').read_text())
        def log(rows, suffix):
            lines = []
            for row in rows:
                family = next(f for f in ['continuation', 'snapshot_policy', 'cutter_policy']
                              if row['id'].startswith('mesh_trim_'+f+'_'))
                lines.append('MESH_TRIM_'+family.upper()+'_'+suffix+' '+json.dumps(row))
            return '\n'.join(lines)
        commands = log(data['commands'], 'COMMAND')
        getters = log(data['getters'], 'GETTER')
        self.assertEqual(summarize(commands, getters), data)
        self.assertEqual((data['command_matched'], data['getter_matched']), (7, 8))
        for field in ['surface', 'selection', 'ownership', 'status', 'channels']:
            rows = deepcopy(data['commands'])
            row = next(r for r in rows if r['id'] == 'mesh_trim_cutter_policy_deleted_cutter')
            obj = next(o for o in row['after']['objects'] if o['role'] == 'output')
            if field == 'surface':
                obj['mesh']['vertices'][0][0] += 1.
            elif field == 'selection':
                obj['selected'] = True
            elif field == 'ownership':
                row['sources_restored_on_undo'] = False
            elif field == 'status':
                row['success'] = False
            else:
                row['after']['channels']['matched'] = False
            with self.assertRaises(ValueError, msg=field):
                summarize(log(rows, 'COMMAND'), getters)
        for rows in [data['getters'][:-1], data['getters']+[data['getters'][0]]]:
            with self.assertRaises(ValueError):
                summarize(commands, log(rows, 'GETTER'))


if __name__ == '__main__':
    unittest.main()
