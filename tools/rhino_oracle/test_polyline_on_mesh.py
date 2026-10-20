"""Fixed mesh getter inputs, immutable native evidence and bounded staging."""
import copy,gzip,hashlib,json,os,tempfile
from pathlib import Path
from unittest import TestCase,mock
from . import polyline_on_mesh_probe as probe
from .client import OracleClient,OracleProtocolError
from .structural_sdk import stage_helpers
HERE=Path(__file__).resolve().parent
class MeshPolylineTests(TestCase):
 def test_fixed_capture_and_source_provenance(self):
  path=HERE/'fixtures/polyline_on_mesh.json';q=json.loads(path.read_text());self.assertEqual(q,probe.request());probe.validate_request(q)
  r=json.loads((HERE/'observations/polyline_on_mesh.json').read_text());self.assertEqual(len(r['results']),12)
  self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
  for name,sha in r['provenance']['sources'].items():
   data=(HERE/name).read_bytes()
   if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
   self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
  for op,row in zip(q['operations'],r['results']):
   self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row)
   v=row['value'];trace=v['input_trace'];self.assertEqual(trace['errors'],[]);self.assertEqual(trace['completed'],trace['expected'])
   parent=next(o for o in v['before']if o['role']=='target')
   for phase in ['after_script','undo','redo']:
    expected=dict(parent,selected=False)if op['case']=='cancel'else parent
    self.assertEqual(next(o for o in v[phase]if o['role']=='target'),expected)
   for o in v['after_script']:
    if o['role']=='output':self.assertIsNotNone(o['polyline']);self.assertFalse(o['selected']);self.assertEqual(o['layer'],'Mesh polyline output')
 def test_inputs_are_bounded_fixed_and_not_aliased(self):
  before=probe.request()
  for field in ['iterations','point','case','duplicate','extra']:
   q=copy.deepcopy(before);op=q['operations'][0]
   if field=='iterations':q['iterations']=2
   elif field=='point':op['actions'][0][0]+=.1
   elif field=='case':op['case']='arbitrary'
   elif field=='duplicate':q['operations'][1]['id']=op['id']
   else:op['script']='arbitrary'
   with self.assertRaises(ValueError):probe.validate_request(q)
  self.assertEqual(before,probe.request())
 def test_private_owned_capture_is_required(self):
  with mock.patch.dict(os.environ,{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as spawn:
   with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(probe.request())
   spawn.assert_not_called()
 def test_complete_helper_closure(self):
  with tempfile.TemporaryDirectory()as directory:
   dest=Path(directory);stage_helpers(probe.request(),HERE,dest)
   for name in ['polyline_on_mesh_probe','conic_getter_input','join_probe','merge_edges_probe','managed_mouse_callback','snap_environment','number_token']:
    self.assertEqual((dest/(name+'.py')).read_bytes(),(HERE/(name+'.py')).read_bytes())

 def test_report_retains_compatibility_gaps_and_current_source_hashes(self):
  root=HERE.parents[1];r=json.loads((root/'docs/mesh-polyline-qualification.json').read_text())
  self.assertFalse(r['full_native_parity']);self.assertEqual(r['model_epsilon'],1e-6)
  self.assertEqual(r['command_cases']['qualified_recipes'],9);self.assertEqual(r['getter_cases']['qualified_recipes'],10)
  self.assertEqual(set(r['getter_cases']['unqualified_recipes']),{'off_mesh','disconnected'})
  for path,sha in r['sha256'].items():self.assertEqual(hashlib.sha256((root/path).read_bytes()).hexdigest(),sha,path)
