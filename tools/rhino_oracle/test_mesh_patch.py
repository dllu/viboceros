"""Fixed MeshPatch input closure, owned captures, source provenance and replay gates."""
from tools.verify_provenance import verify_measurement
import copy,gzip,hashlib,json,os,tempfile,unittest
from pathlib import Path
from unittest import mock
from . import mesh_patch_probe as probe
from .client import OracleClient,OracleProtocolError
from .structural_sdk import stage_helpers,validate_operations
from .report_mesh_patch import summarize
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
class MeshPatchTests(unittest.TestCase):
 def test_native_capture_is_complete_fixed_and_source_pinned(self):
  path=HERE/'fixtures/mesh_patch.json';q=json.loads(path.read_text());self.assertEqual(q,probe.request());validate_operations(q)
  r=json.loads((HERE/'observations/mesh_patch.json').read_text());self.assertEqual(len(r['results']),21)
  self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
  for name,sha in r['provenance']['sources'].items():
   data=(HERE/name).read_bytes()
   if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
   self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
  for op,row in zip(q['operations'],r['results']):
   self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row);v=row['value']
   events=[e for e in v['events']if e['name']=='MeshPatch'];self.assertEqual(len(events),1)
   failure=bool(v['input'].get('expect_failure'));self.assertEqual(events[0]['result'],'Failure'if failure else'Success')
   before=[o for o in v['before']if o['role']!='output']
   for phase in ['after_script','undo','redo']:
    parents=[o for o in v[phase]if o['role']!='output'];self.assertEqual(parents,[dict(o,selected=False)for o in before])
    outputs=[o for o in v[phase]if o['role']=='output'];self.assertEqual(len(outputs),int(not failure and phase!='undo'))
    for o in outputs:self.assertEqual(o['layer'],'MeshPatch output');self.assertIsNone(o['name']);self.assertFalse(o['selected'])
 def test_arbitrary_scripts_or_geometry_cannot_enter_the_fixed_probe(self):
  before=probe.request()
  for field in ['id','case','op','extra']:
   q=probe.request();op=q['operations'][0]
   if field=='extra':op['script']='arbitrary'
   else:op[field]='arbitrary'
   with self.assertRaises(ValueError):validate_operations(q)if field!='op'else probe.validate(op)
  self.assertEqual(before,probe.request())
 def test_capture_requires_private_owned_one_iteration_and_bounded_ids(self):
  with mock.patch.dict(os.environ,{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as spawn:
   with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(probe.request())
   client=OracleClient(settings_scheme='VibocerosOracleMeshPatchTest20261020')
   for kind in ['repeat','boolean','count','duplicate']:
    q=probe.request()
    if kind=='repeat':q['iterations']=2
    elif kind=='boolean':q['iterations']=True
    elif kind=='count':q['operations']*=2
    else:q['operations'].append(copy.deepcopy(q['operations'][0]))
    with self.assertRaises(OracleProtocolError):client.run_rhino(q)
   spawn.assert_not_called()
 def test_staged_helper_closure_is_complete(self):
  with tempfile.TemporaryDirectory()as directory:
   stage_helpers(probe.request(),HERE,Path(directory))
   for name in ['mesh_patch_probe','join_probe','merge_edges_probe']:self.assertEqual((Path(directory)/(name+'.py')).read_bytes(),(HERE/(name+'.py')).read_bytes())
 def test_qualification_retains_limits_and_historical_sources(self):
  r=json.loads((ROOT/'docs/mesh-patch-qualification.json').read_text());self.assertEqual(r['model_epsilon'],1e-6);self.assertFalse(r['full_native_parity']);self.assertFalse(r['performance_qualified']);self.assertEqual(r['native_construction_failures'],['duplicate_constraint'])
  for key in ['geometry_cases','command_cases','getter_cases']:self.assertEqual(r[key]['qualified_recipes'],21)
  self.assertGreater(verify_measurement(r), 0)
 def test_report_rejects_missing_duplicate_nonfinite_and_hidden_failure_results(self):
  r=json.loads((ROOT/'docs/mesh-patch-qualification.json').read_text());lines=[prefix+' '+json.dumps(row)for key,prefix in [('geometry_cases','MESH_PATCH_GEOMETRY'),('command_cases','MESH_PATCH_COMMAND'),('getter_cases','MESH_PATCH_GETTER')]for row in r[key]['cases']]
  summarize('\n'.join(lines))
  for invalid in [lines[:-1],lines+lines[-1:]]:
   with self.assertRaises(ValueError):summarize('\n'.join(invalid))
  for error in [1e-6,float('nan'),float('inf'),-1.]:
   row=copy.deepcopy(r['getter_cases']['cases'][-1]);row['maximum_bidirectional_sample_error']=error;invalid=lines[:];invalid[-1]='MESH_PATCH_GETTER '+json.dumps(row)
   with self.assertRaises(ValueError):summarize('\n'.join(invalid))
  invalid=[line.replace('"native_failure": true','"native_failure": false')for line in lines]
  with self.assertRaises(ValueError):summarize('\n'.join(invalid))
