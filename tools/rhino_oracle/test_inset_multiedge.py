"""Owned smooth-edge partitions, surviving seams and output/history evidence."""
import gzip
import hashlib
import json
import math
import unittest
from .capture_family import HERE
from .generate_inset_surface import request
from .inset_multiedge_probe import cases,validate,PARTITIONS


def native(family):
    return json.loads(gzip.decompress((HERE / ('observations/' + family + '.json.gz')).read_bytes()))


class MultiEdgeInsetTests(unittest.TestCase):
    def test_owned_requests_and_immutable_producers(self):
        for sdk in (False,True):
            family='inset_multiedge_sdk' if sdk else 'inset_multiedge_command'
            query=request();query['operations']=cases(sdk)
            path=HERE / ('fixtures/' + family + '.json');data=native(family)
            self.assertEqual(json.loads(path.read_bytes()),query)
            self.assertEqual([r['id'] for r in data['results']],[op['id'] for op in cases(sdk)])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for sha in data['provenance']['sources'].values():
                source=gzip.decompress((HERE / ('observations/source_snapshots/' + sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(source).hexdigest(),sha)
            for r in data['results']:self.assertNotIn('error',r)

    def test_curved_g2_joins_stay_annular_and_surviving_seams_remain_vertices(self):
        good=rejected=0
        for op,r in zip(cases(True),native('inset_multiedge_sdk')['results']):
            v=r['value'];self.assertTrue(v['source_unchanged'])
            n=len(PARTITIONS[op['partition']])+1
            self.assertEqual(len(v['before']['geometry']['edges']),n)
            if v['output'] is None:
                self.assertTrue(op['scene']=='ellipse' and op['distance']==4. or op['scene']=='bezier' and op['distance']==3.5)
                rejected+=1;continue
            good+=1;self.assertTrue(v['output']['valid']);g=v['output']['geometry']
            cap=1 if op['distance'] in (0.3,2.2) else 3 if op['partition']=='shifted' else 2
            self.assertEqual(tuple(len(g[k]) for k in ('faces','edges','vertices')),(2,n+cap,n+cap))
            if op['partition']=='shifted' and op['distance']==3.:
                normal=(1/math.sqrt(3.25),1.5/math.sqrt(3.25))
                expected=(6/math.sqrt(2)-3*normal[0],4/math.sqrt(2)-3*normal[1])
                self.assertTrue(any(math.hypot(p['point'][0]-expected[0],p['point'][1]-expected[1])<1e-6 for p in g['vertices']))
        self.assertEqual((good,rejected),(28,12))

    def test_commands_match_all_modes_and_keep_failed_sources(self):
        good=rejected=0
        for op,r in zip(cases(),native('inset_multiedge_command')['results']):
            v=r['value'];self.assertNotIn('Unknown command:',v['history'])
            if v['results']==['Failure']:
                rejected+=1;self.assertEqual(len(v['after']),1)
                self.assertEqual(v['before'][0]['brep'],v['after'][0]['brep']);continue
            good+=1;self.assertEqual(v['results'],['Success'])
            self.assertEqual(len(v['after']),1 if op['split'] and op['join_outputs'] else 2)
            self.assertEqual(v['before'][0]['brep'],v['undo'][0]['brep'])
            self.assertEqual(v['after_script'],v['redo'])
            if not op['split']:
                self.assertEqual(v['before'][0]['brep'],v['after'][0]['brep'])
                self.assertIn('definition',v['after'][1]['curve'])
            elif not op['join_outputs']:
                self.assertTrue(all(o['source'] is None for o in v['after']))
        self.assertEqual((good,rejected),(78,36))

    def test_unowned_and_malformed_recipes_decline(self):
        for op in [cases()[0],cases(True)[0]]:
            validate(op)
            for changes in ({'id':'unowned'},{'distance':3},{'crease_corners':1},{'loose':0},{'partition':'unknown'},{'extra':True}):
                with self.assertRaises(ValueError):validate(dict(op,**changes))


if __name__=='__main__':unittest.main()
