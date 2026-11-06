"""Immutable PlanarSrf evidence, exact source recipes and geometric grouping."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .planar_regions_cases import pilot,holdouts,transforms,planes
from .structural_sdk import stage_helpers,validate_operations
from .subd_edge_analysis_replay import HERE,read

FAMILIES=dict(planar_regions_pilot=pilot,planar_regions_holdouts=holdouts,planar_regions_transforms=transforms,planar_regions_confirmations=transforms,planar_regions_planes=planes)


class PlanarRegionTests(unittest.TestCase):
    def test_requests_and_frozen_producers(self):
        for family,make in FAMILIES.items():
            path=HERE/('fixtures/'+family+'.json');query=json.loads(path.read_bytes());data=read(family)
            self.assertEqual(query['operations'],make());validate_operations(query)
            self.assertEqual([r['id']for r in data['results']],[o['id']for o in make()])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for name,sha in data['provenance']['sources'].items():
                self.assertEqual(hashlib.sha256(gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())).hexdigest(),sha,name)

    def test_commands_preserve_sources_and_complete_history(self):
        count=0
        for family in FAMILIES:
            for row in read(family)['results']:
                count+=1;v=row['value'];self.assertTrue(v['success'],row['id'])
                self.assertNotIn('Unknown command:',v['history']);self.assertEqual([e['result']for e in v['events']],['Success'])
                self.assertEqual(v['created'],v['after']);self.assertEqual(v['before'],v['undo']);self.assertEqual(v['after'],v['redo'])
                self.assertEqual(v['before'],[o for o in v['after']if o['source']is not None])
                for out in [o for o in v['after']if o['kind']=='brep']:
                    self.assertEqual(out['faces'],1);self.assertEqual(out['selected'],0)
                    self.assertEqual(out['loops'][0]['kind'],'Outer')
                    for loop in out['loops']:
                        if family!='planar_regions_transforms':self.assertTrue(loop['sources'])
        self.assertEqual(count,190)

    def test_grouping_depends_on_seams_and_keeps_boundary_contacts_independent(self):
        rows={r['id']:r['value']for r in read('planar_regions_holdouts')['results']}
        def regions(name):return [tuple(tuple(l['sources'])for l in o['loops'])for o in rows[name]['after']if o['kind']=='brep']
        self.assertEqual(regions('precise_old_overlap'),[((0,),(1,),(3,)),((2,),)])
        self.assertEqual(regions('precise_seam_00'),[((0,),(1,)),((2,),)])
        self.assertEqual(regions('precise_seam_02'),[((0,),),((1,),),((2,),)])
        for name in ['touch01','cross00','internal_circle_seam1']:self.assertEqual(regions(name),[((0,),),((1,),)])
        self.assertEqual(regions('precise_duplicates'),[((0,1),)])
        self.assertEqual(regions('precise_two_outers_common_hole'),[((0,),),((1,),),((2,),)])
        for name in ['precise_circles','precise_circle_internal_tangent']:
            import math
            areas=sum(o['area']for o in rows[name]['after']if o['kind']=='brep')
            expected=100*math.pi if name=='precise_circles' else 116*math.pi
            self.assertLess(abs(areas-expected),1e-9)

    def test_distinct_polygon_encodings_and_equivalent_circles(self):
        rows={r['id']:r['value']for r in read('planar_regions_confirmations')['results']}
        for kind in ['outer','hole','circle']:
            for encoding in ['reverse','domain','seam','gauge']:
                outputs=[o for o in rows['duplicate_'+kind+'_'+encoding]['after']if o['kind']=='brep']
                if kind=='outer':self.assertEqual(len(outputs),2 if encoding in ['reverse','seam']else 1)
                if kind=='hole':self.assertEqual(len(outputs[0]['loops']),3 if encoding in ['reverse','seam']else 2)
                if kind=='circle':self.assertEqual(len(outputs),1)

    def test_original_large_circle_matcher_diagnostic_is_preserved(self):
        old=read('planar_regions_transforms');new=read('planar_regions_confirmations')
        missing=[]
        for original,confirmed in zip(old['results'],new['results'],strict=True):
            self.assertEqual(original['id'],confirmed['id'])
            for a,b in zip(original['value']['after'],confirmed['value']['after'],strict=True):
                if a['kind']!='brep':continue
                self.assertEqual(a['brep'],b['brep'])
                for x,y in zip(a['loops'],b['loops'],strict=True):
                    if not x['sources']:missing.append(original['id']);self.assertEqual(y['sources'],[0])
        self.assertEqual(missing,['circles_large'])

    def test_malformed_owned_recipes_are_rejected(self):
        for edit in [dict(id='unknown'),dict(extra=True),dict(curves=[])]:
            with self.assertRaises(ValueError):validate_operations(dict(operations=[dict(pilot()[0],**edit)]))
        op=copy.deepcopy(pilot()[0]);op['curves'][0]['control_points'][0]['weight']=True
        with self.assertRaises(ValueError):validate_operations(dict(operations=[op]))

    def test_probe_imports_in_isolation(self):
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(dict(operations=[holdouts()[0]]),HERE,Path(directory))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import planar_regions_probe',directory],check=True,capture_output=True)


if __name__=='__main__':unittest.main()
