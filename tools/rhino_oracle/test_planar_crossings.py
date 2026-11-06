"""Frozen crossing-loop command evidence and independent mesh observations."""
import copy
import gzip
import hashlib
import json
import math
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .planar_crossings_cases import pilot,details,holdouts
from .structural_sdk import stage_helpers,validate_operations
from .subd_edge_analysis_replay import HERE,read

FAMILIES=dict(planar_crossings=pilot,planar_crossings_details=details,planar_crossings_holdouts=holdouts,
              planar_crossings_confirmations=holdouts,planar_crossings_unit_domains=holdouts)


class PlanarCrossingsTests(unittest.TestCase):
    def test_requests_and_frozen_producers(self):
        for family,make in FAMILIES.items():
            path=HERE/('fixtures/'+family+'.json');request=json.loads(path.read_bytes());native=read(family)
            self.assertEqual(request['operations'],make());validate_operations(request)
            self.assertEqual([r['id']for r in native['results']],[r['id']for r in make()])
            self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for name,digest in native['provenance']['sources'].items():
                self.assertEqual(hashlib.sha256(gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())).hexdigest(),digest,name)

    def test_native_sources_regions_validity_and_history(self):
        count=0
        for family in FAMILIES:
            for row in read(family)['results']:
                count+=1;v=row['value'];self.assertTrue(v['success'],row['id'])
                self.assertEqual([e['result']for e in v['events']],['Success'])
                self.assertNotIn('Unknown command:',v['history'])
                self.assertEqual(v['created'],v['after']);self.assertEqual(v['before'],v['undo']);self.assertEqual(v['after'],v['redo'])
                self.assertEqual(v['before'],[o for o in v['after']if o['source']is not None])
                for output in [o for o in v['after']if o['kind']=='brep']:
                    self.assertEqual(output['faces'],1);self.assertTrue(output['native_valid'])
                    self.assertTrue(all(loop['sources']for loop in output['loops']))
        self.assertEqual(count,155)

    def test_repeated_regions_are_unchanged_and_unit_charts_map_all_trim_controls(self):
        original=read('planar_crossings_holdouts')['results']
        for family in ['planar_crossings_confirmations','planar_crossings_unit_domains']:
            for a,b in zip(original,read(family)['results'],strict=True):
                self.assertEqual(a['id'],b['id'])
                for x,y in zip(a['value']['after'],b['value']['after'],strict=True):
                    if x['kind']!='brep':continue
                    self.assertEqual(x['brep'],y['brep'])
                    if 'unit_domain_brep'not in y:continue
                    unit=y['unit_domain_brep'];self.assertEqual(unit['edges'],y['brep']['edges'])
                    self.assertEqual(unit['vertices'],y['brep']['vertices']);self.assertEqual(unit['topology'],y['brep']['topology'])
                    for old,new in zip(y['brep']['faces'],unit['faces'],strict=True):
                        definition=old['definition'];changed=new['definition']
                        self.assertEqual(definition['control_points'],changed['control_points'])
                        self.assertEqual(definition['degree'],changed['degree']);self.assertEqual(definition['control_count'],changed['control_count'])
                        self.assertEqual(changed['domain_u'],[0.,1.]);self.assertEqual(changed['domain_v'],[0.,1.])
                        for axis in ['u','v']:
                            start,end=definition['domain_'+axis]
                            for p,q in zip(definition['knots_'+axis],changed['knots_'+axis],strict=True):self.assertLess(abs((p-start)/(end-start)-q),1e-12)
                        for first,second in zip(old['loops'],new['loops'],strict=True):
                            for c,d in zip(first,second,strict=True):
                                for field in ['degree','knots','domain']:self.assertEqual(c['definition'][field],d['definition'][field])
                                for p,q in zip(c['definition']['control_points'],d['definition']['control_points'],strict=True):
                                    self.assertEqual(p['weight'],q['weight'])
                                    for axis,key in enumerate(['domain_u','domain_v']):
                                        start,end=definition[key];expected=(p['point'][axis]-start)/(end-start)
                                        self.assertLess(abs(expected-q['point'][axis]),1e-12)

    def test_small_circle_native_geometry_has_an_explicit_area_discrepancy(self):
        family='planar_crossings_holdouts';request=json.loads((HERE/('fixtures/'+family+'.json')).read_bytes())
        op=next(o for o in request['operations']if o['id']=='crossing_circle10_small')
        value=next(r['value']for r in read(family)['results']if r['id']==op['id'])
        output=next(o for o in value['after']if o['kind']=='brep');brep=output['brep'];face=brep['faces'][0];surface=face['definition']
        self.assertEqual(surface['degree'],[1,1]);self.assertEqual(surface['control_count'],[2,2])
        self.assertTrue(all(p['weight']==1. for p in surface['control_points']))
        for axis in ['u','v']:
            a,b=surface['domain_'+axis];self.assertEqual(surface['knots_'+axis],[a,a,b,b])
        corners=[p['point']for p in surface['control_points']]
        for axis in range(3):self.assertLess(abs(corners[3][axis]-(corners[1][axis]+corners[2][axis]-corners[0][axis])),1e-12)
        for source,edge in zip(op['curves'],brep['edges'],strict=True):
            c=edge['curve']['definition'];self.assertEqual(source['degree'],c['degree']);self.assertEqual(source['knots'],c['knots'])
            for p,q in zip(source['control_points'],c['control_points'],strict=True):
                self.assertEqual(p['weight'],q['weight'])
                for x,y in zip(p['point'],q['point'],strict=True):self.assertLess(abs(x-y),1e-12)
        for loop,topology in zip(face['loops'],brep['topology']['faces'][0]['loops'],strict=True):
            self.assertEqual(len(loop),1);trim=loop[0]['definition'];topology=topology['trims'][0]
            edge=brep['edges'][topology['edge']]['curve']['definition'];points=edge['control_points'][::-1]if topology['reversed']else edge['control_points']
            self.assertEqual(trim['degree'],edge['degree'])
            def knots(c):
                a,b=c['domain'];return [(k-a)/(b-a)for k in c['knots'][1:-1]]
            expected=knots(edge)
            if topology['reversed']:expected=[1.-k for k in reversed(expected)]
            self.assertEqual(knots(trim),expected)
            for p,q in zip(trim['control_points'],points,strict=True):
                self.assertEqual(p['weight'],q['weight'])
                u,v=[(p['point'][i]-surface[key][0])/(surface[key][1]-surface[key][0])for i,key in enumerate(['domain_u','domain_v'])]
                for axis in range(3):
                    lifted=corners[0][axis]+u*(corners[1][axis]-corners[0][axis])+v*(corners[2][axis]-corners[0][axis])
                    self.assertLess(abs(lifted-q['point'][axis]),1e-12)
        gap=68.*math.pi*1e-6-output['area'];self.assertGreater(gap,1e-9);self.assertLess(gap,1.11e-9)
        for family in ['planar_crossings_confirmations','planar_crossings_unit_domains']:
            row=next(r['value']for r in read(family)['results']if r['id']==op['id']);out=next(o for o in row['after']if o['kind']=='brep')
            self.assertLess(abs(out['area']-out['centered_area']),1e-18)
            if 'unit_domain_area'in out:self.assertLess(abs(out['area']-out['unit_domain_area']),1e-18)

    def test_seams_control_crossing_hole_admission(self):
        rows={r['id']:r['value']for r in read('planar_crossings')['results']}
        for name,areas,loops in [('crossing_rect00',[16.,84.],[1,2]),('crossing_rect01',[68.],[3]),('crossing_rect20',[16.,16.,100.],[1,1,1]),('crossing_bars00',[68.],[3])]:
            outputs=[o for o in rows[name]['after']if o['kind']=='brep']
            self.assertEqual(sorted(o['area']for o in outputs),areas)
            self.assertEqual(sorted(len(o['loops'])for o in outputs),loops)

    def test_native_model_area_and_mesh_area_are_distinct(self):
        rows={r['id']:r['value']for r in read('planar_crossings_details')['results']}
        for suffix,model,mesh in [('crossing_rect01',68.,72.),('crossing_bars00',68.,72.),('duplicate',68.,84.),('hole',84.,84.),('outer',100.,100.),('touch',82.,82.),('separate',82.,82.)]:
            key='crossing_details_'+suffix
            output=next(o for o in rows[key]['after']if o['kind']=='brep')
            self.assertEqual(output['area'],model);self.assertEqual(output['coverage']['mesh_area'],mesh)
            self.assertEqual(output['coverage']['mesh_count'],1)
        for value in rows.values():
            output=next(o for o in value['after']if o['kind']=='brep');coverage=output['coverage']
            self.assertEqual(len(coverage['samples']),64)
            self.assertTrue(coverage['meshes'][0]['vertices']);self.assertTrue(coverage['meshes'][0]['faces'])

    def test_unowned_or_modified_samples_are_rejected(self):
        for key,value in [('id','unowned_crossing'),('samples',[[0.,0.,0.]])]:
            op=copy.deepcopy(details()[0]);op[key]=value
            with self.assertRaises(ValueError):validate_operations(dict(operations=[op]))
        op=copy.deepcopy(pilot()[0]);op['curves'][0]['control_points'][0]['weight']=True
        with self.assertRaises(ValueError):validate_operations(dict(operations=[op]))

    def test_helpers_import_in_isolation(self):
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(dict(operations=[details()[0]]),HERE,Path(directory))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import planar_regions_probe,planar_crossings_probe',directory],check=True,capture_output=True)


if __name__=='__main__':unittest.main()
