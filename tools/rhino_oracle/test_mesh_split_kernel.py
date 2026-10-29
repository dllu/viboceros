"""Frozen mesh partitions, public SDK controls and independently checked claims."""
from tools.verify_provenance import reconstruct_measurement
from copy import deepcopy
import json
import unittest
from .mesh_split_cases import ALL_CASES,validate
from .mesh_boolean_comparison import measures
from .report_mesh_split_kernel import ROOT,HERE,native,summarize,validate_ngons


class MeshSplitKernelTests(unittest.TestCase):
    def test_fixed_requests_native_sources_and_public_sdk_controls(self):
        _,expected,_=native()
        self.assertEqual(len(expected),30)
        for name in ALL_CASES:validate(dict(op='mesh_split_command',id=name,case=name))
        for change in [dict(op='arbitrary'),dict(case=[]),dict(id='wrong'),dict(extra=True)]:
            value=dict(op='mesh_split_command',id=next(iter(ALL_CASES)),case=next(iter(ALL_CASES)))
            value.update(change)
            with self.assertRaises(ValueError):validate(value)

    def test_native_ngons_intermediate_results_and_three_cutter_surface_loss(self):
        capture=json.loads((HERE/'observations/mesh_split_controls.json').read_text())
        rows={r['id'].removeprefix('mesh_split_controls_'):r['value']for r in capture['results']}
        parts=lambda name:rows[name]['split_sdk'][0]['parts']
        self.assertEqual([len(p['ngons'])for p in parts('box_ngons_yes')],[4,4])
        self.assertEqual([len(p['ngons'])for p in parts('box_ngons_no')],[0,0])
        for name in ['partial_keep','disjoint_keep','coplanar_drop']:
            self.assertEqual(len(parts(name)),1)
        self.assertFalse(any(o['role']=='output'for o in rows['coplanar_drop']['after']))
        self.assertAlmostEqual(measures([rows['three_cutters']['input']['sources'][0]])[0],52.)
        self.assertAlmostEqual(measures(parts('three_cutters'))[0],50.9)
        value=deepcopy(parts('box_ngons_yes')[0]);value['ngons'][0]['faces'][0]=len(value['faces'])
        with self.assertRaises(ValueError):validate_ngons(value)

    def test_report_rebuild_and_rejection_of_forged_geometry_or_status(self):
        data=json.loads((ROOT/'docs/mesh-split-kernel-qualification.json').read_text())
        log=lambda rows:'\n'.join('MESH_SPLIT_KERNEL '+json.dumps(row)for row in rows)
        self.assertEqual(reconstruct_measurement(data, summarize, log(data['measurements'])),data)
        self.assertEqual((data['matched'],data['replayed']),(28,30))
        for field in ['coordinate','area','volume','orientation','status','ownership','declaration']:
            rows=deepcopy(data['measurements']);row=next(r for r in rows if r['id']=='mesh_split_box_plane_post')
            if field=='coordinate':row['parts'][0]['vertices'][0][0]+=1.
            elif field=='area':row['area']+=1.
            elif field=='volume':row['signed_volume']+=1.
            elif field=='orientation':row['oriented_area'][0]+=1.
            elif field=='status':row['error']='failure'
            elif field=='ownership':row['sources_unchanged']=False
            else:row['matched']=False
            with self.assertRaises(ValueError):summarize(log(rows))
        for rows in [data['measurements'][:-1],data['measurements']+[data['measurements'][0]]]:
            with self.assertRaises(ValueError):summarize(log(rows))


if __name__=='__main__':unittest.main()
