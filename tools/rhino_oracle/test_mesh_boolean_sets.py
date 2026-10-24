"""Multi-set ownership, immutable producers, measured geometry and partition conservation."""
from copy import deepcopy
import json
import unittest
from .mesh_boolean_comparison import measures
from .report_mesh_boolean_sets import ROOT, native, summarize


class MeshBooleanSetTests(unittest.TestCase):
    def test_saved_scenes_and_native_multi_target_output_ownership(self):
        _, rows, exploratory, _ = native()
        self.assertEqual((len(rows),len(exploratory)),(67,2))
        for mode, owner in [('pre',0),('post',1)]:
            for operation in ['difference','intersection']:
                value=rows['boolean_sets_{}_reverse_{}'.format(operation,mode)]
                outputs=[o for o in value['after']if o.get('mesh')and o['name']=='Source '+str(owner)]
                self.assertEqual(len(outputs),2)
                self.assertEqual(outputs[0]['source'],owner)
                self.assertTrue(all(o['color']==[13+owner,27,53] for o in outputs))
        self.assertIn('Tolerance=Document',rows['boolean_split_controls_union_after_split']['history'])
        self.assertEqual([o['source']for o in rows['boolean_split_controls_difference_after_split']['after']if o['role']=='source'],[1,0])

    def test_single_native_split_command_retains_intermediate_partitions_and_empty_outside(self):
        _,rows,_,_=native()
        for name in ['boolean_split_two_cutters','boolean_split_controls_two_cutters_repeat']:
            value=rows[name]
            self.assertEqual(sum(e['name']=='MeshBooleanSplit'for e in value['events']),1)
            parts=[o['mesh']for o in value['after']if o.get('source')==0 or o['role']=='output']
            self.assertEqual(len(parts),5)
            self.assertAlmostEqual(sum(measures([m])[1]for m in parts),48.,places=9)
            self.assertAlmostEqual(measures([value['input']['sources'][0]])[1],24.,places=9)
        triple=rows['boolean_split_stages_three_cutters']
        parts=[o['mesh']for o in triple['after']if o.get('source')==0 or o['role']=='output']
        self.assertEqual(len(parts),12)
        self.assertAlmostEqual(sum(measures([m])[1]for m in parts),96.,places=9)
        self.assertEqual(sum(e['name']=='MeshBooleanSplit'for e in triple['events']),1)
        outside=next(o['mesh']for o in rows['boolean_split_controls_target_inside']['after']if o['source']==1)
        self.assertEqual((outside['vertices'],outside['faces']),([],[]))

    def test_report_recomputes_results_and_rejects_unmeasured_or_forged_claims(self):
        data=json.loads((ROOT/'docs/mesh-boolean-sets-qualification.json').read_text())
        log=lambda key,prefix:'\n'.join(prefix+' '+json.dumps(r)for r in data[key])
        commands=log('commands','MESH_BOOLEAN_SETS_COMMAND')
        getters=log('getters','MESH_BOOLEAN_SETS_GETTER')
        self.assertEqual(summarize(commands,getters),data)
        self.assertEqual((data['command_matched'],data['getter_matched']),(50,53))
        for bad in ['\n'.join(commands.splitlines()[:-1]),commands+'\n'+commands.splitlines()[0]]:
            with self.assertRaises(ValueError):summarize(bad,getters)
        for what in ['metadata','geometry','status','ownership','kernel_failure']:
            changed=deepcopy(data['commands'])
            row=next(r for r in changed if r['id']=='boolean_split_overlap_post')
            if what=='metadata':row['after']['objects'][-1]['name']='forged'
            elif what=='geometry':row['after']['objects'][-1]['mesh']['vertices'][0][0]+=1.
            elif what=='status':row['success']=False
            elif what=='ownership':row['sources_restored_on_undo']=False
            else:row['error']='unqualified kernel error'
            with self.assertRaises(ValueError):
                summarize('\n'.join('MESH_BOOLEAN_SETS_COMMAND '+json.dumps(r)for r in changed),getters)


if __name__=='__main__':unittest.main()
