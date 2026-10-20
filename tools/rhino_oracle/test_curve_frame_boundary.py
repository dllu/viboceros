"""Frame query policy: bounded SDK sources, public getters and retained poles."""
import gzip,hashlib,json,math,subprocess,sys,tempfile
from pathlib import Path
from unittest import TestCase
from .generate_curve_frame_boundary import request as frames
from .generate_curve_array_frames import request as arrays
from .test_curve_array import source
from .structural_sdk import validate_operations,stage_helpers
HERE=Path(__file__).resolve().parent

def recorded(name):return json.loads((HERE/'fixtures'/(name+'.json')).read_text()),json.loads((HERE/'observations'/(name+'.json')).read_text())
def sdk_suites():return [('curve_frame_'+k,frames(k),n)for k,n in[('boundaries',61),('seeds',46),('proximity',54)]]
def array_suites():return [('curve_array_frames',arrays(),44),('curve_array_frames_stable',arrays('stable'),16)]
class FrameQueryTests(TestCase):
    def test_reproducible_requests_native_source_receipts_and_standalone_staging(self):
        for name,q,count in sdk_suites()+array_suites():
            saved,r=recorded(name);self.assertEqual(q,saved);self.assertEqual(len(r['results']),count);self.assertEqual(hashlib.sha256((HERE/'fixtures'/(name+'.json')).read_bytes()).hexdigest(),r['provenance']['request_sha256'])
            for producer,sha in r['provenance']['sources'].items():self.assertEqual(hashlib.sha256(source(producer,sha)).hexdigest(),sha,producer)
            for op,row in zip(q['operations'],r['results']):self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row)
        with tempfile.TemporaryDirectory()as folder:
            q=frames('proximity');stage_helpers(q,HERE,Path(folder));self.assertEqual((Path(folder)/'curve_frame_boundary_probe.py').read_bytes(),(HERE/'curve_frame_boundary_probe.py').read_bytes())
            subprocess.run([sys.executable,'-I','-c','import sys,json;sys.path.insert(0,sys.argv[1]);import curve_frame_boundary_probe as p;p.validate(json.loads(sys.argv[2]))',folder,json.dumps(q['operations'][0])],check=True,capture_output=True)
    def test_all_actual_getters_and_real_history_are_retained(self):
        for name,q,_ in array_suites():
            _,r=recorded(name)
            for op,row in zip(q['operations'],r['results']):
                v=row['value'];t=v['input_trace'];self.assertEqual(t['errors'],[]);self.assertEqual(t['completed'],len(v['actions']));self.assertEqual(t['completed'],t['expected']);self.assertTrue(v['guide_unchanged'])
                for sent in t['sent']:self.assertTrue(any(sent['prompt'].startswith(p)for p in v['action_prompts'][sent['action']]))
                self.assertIn('Undoing ArrayCrv',v['undo_history']);self.assertIn('Redoing ArrayCrv',v['undo_history']);self.assertEqual(v['after_script'],v['redo'])
    def test_native_projection_poles_are_observed_not_discarded_geometry_failures(self):
        q,r=recorded('curve_array_frames');rows=dict((op['id'],row['value'])for op,row in zip(q['operations'],r['results']))
        def columns(v,index):
            clouds=[o['geometry']['points']for o in v['after_script']['objects']if o['role']=='output'];p=clouds[index];return[[x-y for x,y in zip(p[i],p[0])]for i in[1,2,3]]
        for a,b,seed,tangent in[('frame_array_13','frame_array_15',[0.,-1.,1.],[-2.,2.,-1.]),('frame_array_25','frame_array_27',[1.,1.,0.],[-2.,-1.,2.])]:
            norm=math.sqrt(sum(x*x for x in seed));seed=[x/norm for x in seed];c=columns(rows[a],1);x=[sum(c[j][i]*seed[j]for j in range(3))for i in range(3)]
            cross=[x[1]*tangent[2]-x[2]*tangent[1],x[2]*tangent[0]-x[0]*tangent[2],x[0]*tangent[1]-x[1]*tangent[0]]
            self.assertLess(math.sqrt(sum(v*v for v in cross)),1e-14)
            ca,cb=columns(rows[a],-1),columns(rows[b],-1);self.assertGreater(max(abs(x-y)for p,q in zip(ca,cb)for x,y in zip(p,q)),.5)
        _,r=recorded('curve_frame_proximity');a=r['results'][16]['value'];b=r['results'][30]['value'];self.assertEqual(a['curve'],b['curve']);self.assertNotEqual(a['frames'][1]['z'],b['frames'][1]['z'])
    def test_invalid_sdk_inputs_are_bounded(self):
        op=frames('boundaries')['operations'][0]
        for patch in[dict(extra=True),dict(id=1),dict(vertices=[]),dict(vertices=[[0,0,0],[0,0,0]]),dict(vertices=[[0,0,0],[math.inf,0,0]]),dict(representation='Unknown'),dict(representation='Bezier',vertices=[[0,0,0],[1,0,0],[2,1,0]]),dict(parameters=None),dict(parameters=[0,0]),dict(parameters=[0,4]),dict(parameters=list(range(130))),dict(query_mode='Unknown'),dict(query_mode='Knots',parameters=[0,-1]),dict(query_mode='Knots',parameters=[0,1.5]),dict(query_mode='Knots',parameters=[0,4]),dict(domain=[1,1])]:
            with self.subTest(patch=patch),self.assertRaises(ValueError):validate_operations(dict(operations=[dict(op,**patch)]))
    def test_pilots_keep_their_exact_public_producers(self):
        for name,count in[('curve-frame-boundary-pilot',28),('curve-frame-seed-pilot',21),('curve-frame-proximity-pilot',52),('curve-frame-proximity-protocol-pilot',52)]:
            d=json.loads(gzip.decompress((HERE.parent.parent/'docs'/(name+'.json.gz')).read_bytes()));self.assertEqual(len(d['request']['operations']),count)
            for n,sha in d['response']['provenance']['sources'].items():self.assertEqual(hashlib.sha256(source(n,sha)).hexdigest(),sha,n)
    def test_validation_receipt_pins_artifacts_implementation_and_every_replay(self):
        receipt=json.loads((HERE.parent.parent/'docs/curve-frame-query-validation.json').read_text());self.assertEqual(receipt['array_workflows'],122);self.assertEqual(receipt['sdk_batches'],161)
        for group in['implementation_sources','artifacts']:
            for name,sha in receipt[group].items():
                path=HERE.parent.parent/name;data=path.read_bytes()
                if group=='implementation_sources'and hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
        for name,rows in receipt['array_replays'].items():
            self.assertEqual(len(rows),122,name);qualified=[r for r in rows if r['qualification']=='qualified'];self.assertEqual(len(qualified),99);self.assertLessEqual(max(r['maximum_error']for r in qualified),1e-6)
        for name,rows in receipt['sdk_replays'].items():
            self.assertEqual(len(rows),161,name);self.assertEqual(sum(r['qualification']=='qualified'for r in rows),159);self.assertLessEqual(max(r['maximum_error']for r in rows if r['qualification']=='qualified'),1e-6)
