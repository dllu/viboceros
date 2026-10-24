"""Audit frozen self-intersection inputs, state, geometry and report claims."""
from copy import deepcopy
import json
import unittest
from .mesh_self_intersect_cases import ALL_CASES,validate
from .report_mesh_self_intersect import ROOT,native,summarize


class MeshSelfIntersectTests(unittest.TestCase):
    def test_fixed_catalog_and_immutable_native_inputs(self):
        _,rows,_=native()
        self.assertEqual(len(rows),39)
        for name in ALL_CASES:validate(dict(op='mesh_self_intersect_command',id=name,case=name))
        for change in [dict(op='arbitrary'),dict(case=[]),dict(id='wrong'),dict(extra=True)]:
            value=dict(op='mesh_self_intersect_command',id=next(iter(ALL_CASES)),case=next(iter(ALL_CASES)))
            value.update(change)
            with self.assertRaises(ValueError):validate(value)

    def test_native_adjacency_per_object_scope_and_shared_preferences(self):
        _,rows,_=native()
        outputs=lambda name:[o for o in rows['mesh_self_intersect_'+name]['after']if o['role']=='output']
        for name in ['flat_post','box_post','adjacent_post','separate_objects_post',
                     'controls_flat_chain','controls_edge_contact','controls_after_drop_cancel','controls_mesh_mode']:
            self.assertEqual(outputs(name),[],name)
        for name in ['crossing_post','coplanar_post','folded_post','shared_vertex_post','identical_faces_post']:
            self.assertEqual(len(outputs(name)),1,name)
        self.assertEqual(len(outputs('two_self_meshes_post')),2)
        self.assertIn('Tolerance=Zero',rows['mesh_self_intersect_controls_intersect_after_zero']['history'])
        self.assertIn('Tolerance=Document',rows['mesh_self_intersect_controls_after_boolean_document']['history'])
        self.assertIn('CoplanarFaces=Drop',rows['mesh_self_intersect_controls_after_drop_cancel']['history'])

    def test_report_recomputes_geometry_status_selection_and_source_ownership(self):
        data=json.loads((ROOT/'docs/mesh-self-intersect-qualification.json').read_text())
        log=lambda rows,prefix:'\n'.join(prefix+' '+json.dumps(r)for r in rows)
        commands=log(data['commands'],'MESH_SELF_INTERSECT_COMMAND')
        getters=log(data['getters'],'MESH_SELF_INTERSECT_GETTER')
        self.assertEqual(summarize(commands,getters),data)
        self.assertEqual((data['command_matched'],data['getter_matched']),(34,37))
        for field in ['coordinate','length','selection','ownership','status']:
            rows=deepcopy(data['commands']);row=next(r for r in rows if r['id']=='mesh_self_intersect_crossing_post')
            obj=next(o for o in row['after']['objects']if o['role']=='output')
            if field=='coordinate':obj['polyline'][0][0]+=1.
            elif field=='length':obj['polyline'].insert(1,[99.,99.,99.])
            elif field=='selection':obj['selected']=False
            elif field=='ownership':row['sources_restored_on_undo']=False
            else:row['success']=False
            with self.assertRaises(ValueError):summarize(log(rows,'MESH_SELF_INTERSECT_COMMAND'),getters)
        for rows in [data['commands'][:-1],data['commands']+[data['commands'][0]]]:
            with self.assertRaises(ValueError):summarize(log(rows,'MESH_SELF_INTERSECT_COMMAND'),getters)


if __name__=='__main__':unittest.main()
