"""Actual SubD area commands, mode controls and immutable producer provenance."""
import copy,gzip,hashlib,json,subprocess,sys,tempfile,unittest
from pathlib import Path
from .subd_area_cases import cases,followups,source
from .structural_sdk import validate_operations,stage_helpers
HERE=Path(__file__).resolve().parent
FAMILIES=[('discovery',cases()),('followups',followups())]

def read(name):
    p=HERE/'observations'/(name+'.json')
    return json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes()))

class SubDAreaTests(unittest.TestCase):
    def test_owned_commands_sources_and_frozen_provenance(self):
        count=0
        for family,recipes in FAMILIES:
            name='subd_area_'+family;fixture=HERE/'fixtures'/(name+'.json')
            request=json.loads(fixture.read_bytes());validate_operations(request)
            self.assertEqual(request['operations'],recipes)
            data=read(name)
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(fixture.read_bytes()).hexdigest())
            for name,digest in data['provenance']['sources'].items():
                raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)
            self.assertEqual(len(data['results']),len(recipes))
            for op,row in zip(recipes,data['results'],strict=True):
                self.assertEqual(op['id'],row['id']);v=row['value'];self.assertTrue(v['source_unchanged'])
                expected=source(op['scene']);self.assertEqual(expected.keys(),v['input'].keys())
                for key in expected:
                    if key!='points':self.assertEqual(expected[key],v['input'][key])
                for a,b in zip(expected['points'],v['input']['points'],strict=True):
                    for x,y in zip(a,b,strict=True):self.assertAlmostEqual(x,y,delta=1e-12)
                self.assertEqual([c['command']for c in v['commands']],['Area','AreaCentroid'])
                for c in v['commands']:
                    count+=1;self.assertTrue(c['success']);self.assertNotIn('Unknown command',c['history'])
                    self.assertEqual(c['after']['selected'],op.get('preselect',True))
                    self.assertEqual(c['before']['objects'],1+op.get('curve',False))
                    self.assertEqual(c['after']['objects'],c['before']['objects']+(c['command']=='AreaCentroid'))
        self.assertEqual(count,38)

    def test_extraordinary_controls_distinguish_g1_from_g1x(self):
        for row in read('subd_area_followups')['results']:
            if row['id'] not in ('fan7','asymmetric8'):continue
            v=row['value'];d={r['mode']:r['area']for r in v['diagnostics']}
            self.assertGreater(abs(d[1]-d[3]),.001)
            history=v['commands'][0]['history'];value=float(history.split('Area = ',1)[1].split()[0])
            self.assertAlmostEqual(value,d[3],delta=1e-6)

    def test_tight_reference_does_not_replace_default_native_comparison(self):
        import math,re
        from .subd_area_replay import compare
        for row in read('subd_area_followups')['results']:
            if row['id'] not in ('bent_grid_mixed','cube_mixed'):continue
            native=row['value'];actual=copy.deepcopy(native)
            mass=next(m for m in native['diagnostics']if m['mode']==3);circle=4*math.pi
            point=[(mass['area']*x+circle*y)/(mass['area']+circle)for x,y in zip(mass['centroid'],[20,0,0],strict=True)]
            for c in actual['commands']:
                c['source_unchanged']=True
                if c['command']=='Area':c['area']=float(re.search(r'^(?:Cumulative )?Area = ([0-9.e+-]+)',c['history'],re.M)[1])
                else:
                    for phase in ('after','redo'):c[phase]['points']=[point]
            rows=compare(dict(id=row['id'],curve=True),native,actual)
            self.assertTrue(rows[0]['native_pass']);self.assertFalse(rows[1]['native_pass'])
            self.assertTrue(rows[1]['structural']);self.assertTrue(rows[1]['tight_mixed'])

    def test_recipe_validation_and_isolated_imports(self):
        for _,recipes in FAMILIES:
            for op in recipes:
                for change in [dict(flat=1),dict(curve=0),dict(preselect=1),dict(scene='external'),dict(extra=True)]:
                    with self.assertRaises(ValueError):validate_operations(dict(operations=[dict(copy.deepcopy(op),**change)]))
        with tempfile.TemporaryDirectory()as folder:
            stage_helpers(dict(operations=cases()+followups()),HERE,Path(folder))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import subd_area_probe',folder],check=True,capture_output=True)

if __name__=='__main__':unittest.main()
