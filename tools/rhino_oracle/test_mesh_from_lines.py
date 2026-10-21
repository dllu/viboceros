"""Fixed line networks, private capture closure and complete qualification."""
import copy,gzip,hashlib,json,os,tempfile,unittest
from pathlib import Path
from unittest import mock
from . import mesh_from_lines_probe as probe
from .client import OracleClient,OracleProtocolError
from .structural_sdk import validate_operations,stage_helpers
from .report_mesh_from_lines import GROUPS,summarize
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]

class MeshFromLinesTests(unittest.TestCase):
 def test_fixed_inputs_native_sources_and_immutable_provenance(self):
  factories=[probe.request,probe.variant_request,probe.control_request,probe.edge_request]
  for (_,_,suffix,count),factory in zip(GROUPS,factories):
   path=HERE/f'fixtures/mesh_from_lines{suffix}.json';q=json.loads(path.read_text());self.assertEqual(q,factory());validate_operations(q)
   r=json.loads((HERE/f'observations/mesh_from_lines{suffix}.json').read_text());self.assertEqual(len(r['results']),count);self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
   for name,sha in r['provenance']['sources'].items():
    data=(HERE/name).read_bytes()
    if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
    self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
   for op,row in zip(q['operations'],r['results']):
    self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row);v=row['value'];self.assertEqual(v['input'],probe.SCENES[op['case']]);self.assertTrue(v['sdk_inputs_unchanged'])
    events=[e for e in v['events']if e['name']=='MeshFromLines'];self.assertEqual(len(events),1);self.assertIn(events[0]['result'],['Success','Nothing'])
    before=v['before'];self.assertEqual([o['ends']for o in before[:len(v['input']['lines'])]],v['input']['lines'])
    for phase in ['after_script','undo','redo']:
     for source in [o for o in v[phase]if o['role']=='source']:
      self.assertTrue(any(o['crc']==source['crc']and o['ends']==source['ends']for o in before))
     for output in [o for o in v[phase]if o['role']=='output']:
      self.assertIsNone(output['name']);self.assertFalse(output['selected']);self.assertEqual(output['layer'],'MeshFromLines output')
 def test_real_gap_controls_move_one_endpoint_and_preserve_the_original(self):
  controls=probe.CONTROLS
  for name in ['actual_gap_small','actual_gap_equal','actual_gap_large','actual_gap_huge','actual_sdk_tight','actual_sdk_loose']:
   case=controls[name];self.assertEqual(case['lines'][0][1],[4.,0.,0.]);self.assertNotEqual(case['lines'][0][1],case['lines'][1][0])
  before=copy.deepcopy(probe.QUAD);q=probe.control_request();q['operations'][0]['case']='changed';self.assertEqual(probe.QUAD,before);self.assertNotEqual(q,probe.control_request())
 def test_only_named_bounded_private_one_iteration_requests_can_launch(self):
  for mutation in [{'id':'arbitrary'},{'case':'arbitrary'},{'script':'arbitrary'},{'lines':[]}]:
   q=probe.request();q['operations'][0].update(mutation)
   with self.assertRaises(ValueError):validate_operations(q)
  with mock.patch.dict(os.environ,{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as spawn:
   with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(probe.request())
   client=OracleClient(settings_scheme='VibocerosOracleMeshFromLinesTest20261021')
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
   for name in ['mesh_from_lines_probe','join_probe','merge_edges_probe']:self.assertEqual((Path(directory)/(name+'.py')).read_bytes(),(HERE/(name+'.py')).read_bytes())
 def lines(self):
  r=json.loads((ROOT/'docs/mesh-from-lines-qualification.json').read_text())
  return r,[prefix+'_'+stage.upper()+' '+json.dumps(row)for name,prefix,_,_ in GROUPS for stage in ['geometry','command','getter']for row in r['groups'][name][stage]['cases']]
 def test_qualification_retains_every_case_current_source_and_rejected_token(self):
  r,lines=self.lines();self.assertEqual(summarize('\n'.join(lines)),r);self.assertEqual(r['recipes'],60);self.assertEqual(r['model_epsilon'],1e-6);self.assertFalse(r['full_native_parity']);self.assertFalse(r['performance_qualified'])
  self.assertEqual(set(r['native_inline_option_gaps']),{'current_layer','input_layer','source_layer','layer_index','layer_input_name'})
  self.assertEqual(sum(g['geometry']['qualified_recipes']for g in r['groups'].values()),60)
  for stage in ['command','getter']:self.assertEqual(sum(g[stage]['qualified_recipes']for g in r['groups'].values()),55)
  for path,sha in r['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),sha,path)
 def test_report_rejects_missing_duplicate_nonfinite_and_hidden_token_rows(self):
  r,lines=self.lines()
  for invalid in [lines[:-1],lines+lines[-1:]]:
   with self.assertRaises(ValueError):summarize('\n'.join(invalid))
  for error in [1e-6,-1.,True,float('nan'),float('inf')]:
   row=copy.deepcopy(r['groups']['base']['geometry']['cases'][0]);row['maximum_bidirectional_sample_error']=error;invalid=lines[:];invalid[0]='MESH_FROM_LINES_GEOMETRY '+json.dumps(row)
   with self.assertRaises(ValueError):summarize('\n'.join(invalid))
  index=next(i for i,s in enumerate(lines)if s.startswith('MESH_FROM_LINES_COMMAND ')and not json.loads(s.split(' ',1)[1])['qualified']);row=json.loads(lines[index].split(' ',1)[1]);row['qualified']=True;row['native_options_supported']=True;invalid=lines[:];invalid[index]='MESH_FROM_LINES_COMMAND '+json.dumps(row)
  with self.assertRaises(ValueError):summarize('\n'.join(invalid))
