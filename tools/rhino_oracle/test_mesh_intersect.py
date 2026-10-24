"""Independent MeshIntersect geometry, options, immutable inputs and report audits."""
from copy import deepcopy
import json
import unittest
from .mesh_intersect_cases import ALL_CASES, validate
from .report_mesh_intersect import ROOT, native, summarize


class MeshIntersectTests(unittest.TestCase):
    def test_fixed_inputs_and_archived_native_command_scenes(self):
        _,rows,exploratory,_=native()
        self.assertEqual((len(rows),len(exploratory)),(60,3))
        for name in ALL_CASES:validate(dict(op='mesh_intersect_command',id=name,case=name))
        for change in [dict(op='arbitrary'),dict(case='arbitrary'),dict(id='wrong'),dict(extra=True)]:
            value=dict(op='mesh_intersect_command',id=next(iter(ALL_CASES)),case=next(iter(ALL_CASES)))
            value.update(change)
            with self.assertRaises(ValueError):validate(value)

    def test_native_menu_names_preferences_and_coplanar_output(self):
        _,rows,_,_=native()
        prefix='mesh_intersect_controls_'
        self.assertIn('( Drop  Mesh  Polylines )',rows[prefix+'menu']['history'])
        self.assertIn('Tolerance=Zero',rows[prefix+'boolean_after_zero']['history'])
        self.assertIn('Tolerance=Document',rows[prefix+'after_boolean_document']['history'])
        self.assertIn('CoplanarFaces=Drop',rows[prefix+'after_drop_cancel']['history'])
        self.assertFalse(any(o['role']=='output' for o in rows['mesh_intersect_coplanar_drop']['after']))
        self.assertEqual(sum(o['role']=='output'for o in rows['mesh_intersect_coplanar_polylines']['after']),1)
        self.assertTrue(all(o['selected'] for o in rows['mesh_intersect_overlap_post']['after']if o['role']!='witness'))

    def test_report_recomputes_curves_and_rejects_forged_claims(self):
        data=json.loads((ROOT/'docs/mesh-intersect-qualification.json').read_text())
        log=lambda rows,prefix:'\n'.join(prefix+' '+json.dumps(r)for r in rows)
        commands=log(data['commands'],'MESH_INTERSECT_COMMAND')
        getters=log(data['getters'],'MESH_INTERSECT_GETTER')
        self.assertEqual(summarize(commands,getters),data)
        self.assertEqual((data['command_matched'],data['getter_matched']),(54,57))
        for field in ['coordinate','selection','length','ownership','status']:
            rows=deepcopy(data['commands']);row=next(r for r in rows if r['id']=='mesh_intersect_overlap_post')
            obj=next(o for o in row['after']['objects']if o['role']=='output')
            if field=='coordinate':obj['polyline'][0][0]+=1.
            elif field=='selection':obj['selected']=False
            elif field=='length':obj['polyline'].insert(1,[99.,99.,99.])
            elif field=='ownership':row['sources_restored_on_undo']=False
            else:row['success']=False
            with self.assertRaises(ValueError):summarize(log(rows,'MESH_INTERSECT_COMMAND'),getters)
        for rows in [data['commands'][:-1],data['commands']+[data['commands'][0]]]:
            with self.assertRaises(ValueError):summarize(log(rows,'MESH_INTERSECT_COMMAND'),getters)


if __name__=='__main__':unittest.main()
