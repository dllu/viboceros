"""Owned native SubD volume commands, warning ownership and frozen provenance."""
import copy,gzip,hashlib,json,subprocess,sys,tempfile,unittest
from pathlib import Path
from .subd_volume_cases import cases,open_cases,admission_cases,source
from .structural_sdk import validate_operations,stage_helpers
from .volume_confirmation import VolumeConfirmation
from .client import OracleProtocolError
HERE=Path(__file__).resolve().parent
FAMILIES=[('discovery',cases()),('open',open_cases()),('admission',admission_cases())]

def read(name):
    p=HERE/'observations'/(name+'.json')
    return json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes()))

class SubDVolumeTests(unittest.TestCase):
    def test_native_geometry_commands_warnings_and_immutable_provenance(self):
        count=warnings=admissions=0
        for family,recipes in FAMILIES:
            name='subd_volume_'+family;fixture=HERE/'fixtures'/(name+'.json');request=json.loads(fixture.read_bytes())
            validate_operations(request);self.assertEqual(request['operations'],recipes);data=read(name)
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(fixture.read_bytes()).hexdigest())
            for name,digest in data['provenance']['sources'].items():
                raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)
            self.assertEqual(len(recipes),len(data['results']))
            for op,row in zip(recipes,data['results'],strict=True):
                count+=1;self.assertEqual(row['id'],op['id']);v=row['value'];self.assertTrue(v['source_unchanged'])
                self.assertEqual(v['input'].keys(),source(op['scene']).keys())
                for key in v['input']:
                    if key!='points':self.assertEqual(v['input'][key],source(op['scene'])[key])
                for a,b in zip(v['input']['points'],source(op['scene'])['points'],strict=True):
                    for x,y in zip(a,b,strict=True):self.assertAlmostEqual(x,y,delta=1e-12)
                self.assertNotIn('Unknown command',v['history'])
                self.assertEqual(v['success'],op.get('open_confirmation')!='no')
                self.assertEqual(v['after']['selected'],[op['preselect']]*(1+op['mixed']))
                self.assertEqual(v['before']['objects'],1+op['mixed'])
                self.assertEqual(v['after']['objects'],1+op['mixed']+(op['command']=='VolumeCentroid'and v['success']))
                if 'open_confirmation'in op:
                    warnings+=1;self.assertEqual(v['open_confirmation']['requested'],op['open_confirmation'])
                    self.assertTrue(v['open_confirmation']['dialog']['owner_pid'])
                if op.get('replace_reversed'):
                    candidate=v['replacement']
                    self.assertEqual(candidate['points'],v['constructed']['points'])
                    self.assertEqual(candidate['faces'],[f[:1]+list(reversed(f[1:]))for f in v['constructed']['faces']])
                    self.assertEqual(v['stored'],v['constructed'] if op['scene']=='cube'else candidate)
                if v['constructed']!=v['stored']:
                    admissions+=1;self.assertIn(op['scene'],('reversed_cube','open_cube'))
                    self.assertEqual(v['constructed']['points'],v['stored']['points'])
                    self.assertEqual(v['constructed']['vertex_tags'],v['stored']['vertex_tags'])
                    self.assertEqual([f[:1]+list(reversed(f[1:]))for f in v['constructed']['faces']],v['stored']['faces'])
        self.assertEqual((count,warnings,admissions),(58,20,8))

    def test_high_valence_controls_distinguish_g1x(self):
        row=next(r for r in read('subd_volume_discovery')['results']if r['id']=='bipyramid7_0_1_0_Volume')
        modes={m['mode']:m['volume']for m in row['value']['diagnostics']}
        self.assertGreater(abs(modes[1]-modes[3]),.1)

    def test_fixed_recipes_and_isolated_worker_imports(self):
        for _,recipes in FAMILIES:
            for op in recipes:
                for change in [dict(flat=1),dict(mixed=0),dict(preselect=1),dict(replace_reversed=1),dict(scene='external'),dict(command='Delete'),dict(extra=True)]:
                    with self.assertRaises(ValueError):validate_operations(dict(operations=[dict(copy.deepcopy(op),**change)]))
        request=dict(protocol_version=1,iterations=1,operations=open_cases());VolumeConfirmation(request)
        for key in ('open_confirmation','command'):
            bad=copy.deepcopy(request);bad['operations'][0][key]='unrequested'
            with self.assertRaises(OracleProtocolError):VolumeConfirmation(bad)
        with tempfile.TemporaryDirectory()as folder:
            stage_helpers(request,HERE,Path(folder))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import subd_volume_probe',folder],check=True,capture_output=True)

if __name__=='__main__':unittest.main()
