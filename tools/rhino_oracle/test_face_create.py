"""Fixed 3DFace inputs, immutable oracle evidence and complete qualification."""
from tools.verify_provenance import reconstruct_measurement, verify_measurement
import copy
import gzip
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock
from . import face_create_probe as probe
from .client import OracleClient, OracleProtocolError
from .report_face_create import GROUPS, summarize
from .face_draft_cases import request as draft_request,extra_request,corner_request,corner_control_request,corner_finish_request
from .structural_sdk import stage_helpers, validate_operations
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]


class FaceCreateTests(unittest.TestCase):
 def test_native_recipes_and_immutable_capture_sources(self):
  for (suffix,count),factory in zip(GROUPS,[probe.request,probe.variant_request,probe.control_request,draft_request,extra_request,corner_request,corner_control_request,corner_finish_request]):
   path=HERE/f'fixtures/face_create{suffix}.json';q=json.loads(path.read_text())
   self.assertEqual(q,factory());validate_operations(q)
   response=json.loads((HERE/f'observations/face_create{suffix}.json').read_text())
   self.assertEqual(len(response['results']),count)
   self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),response['provenance']['request_sha256'])
   for name,sha in response['provenance']['sources'].items():
    data=(HERE/name).read_bytes()
    if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
    self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
   for op,row in zip(q['operations'],response['results']):
    self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row)
    value=row['value'];self.assertEqual(value['input'],probe.SCENES[op['case']])
    events=[e for e in value['events']if e['name']=='3DFace'];self.assertEqual(len(events),1)
    self.assertIn(events[0]['result'],['Success','Cancel','Nothing'])
    self.assertNotIn('Unknown command:',value['history'])
    self.assertEqual(value['after_script'],value['redo'])
    if suffix in ('_controls','_drafts','_draft_extras','_corners','_corner_controls','_corner_finish'):
     self.assertGreaterEqual(value['input_trace']['requests'],0)
     if any(isinstance(p,dict)for p in value['input']['points'])or value['input'].get('edge_pick')or value['input'].get('cancel'):self.assertGreater(value['input_trace']['requests'],0)
     self.assertTrue(all(p.startswith('Pick point')for p in value['input_trace']['prompts']))
 def test_captured_native_mesh_validity_and_source_closure(self):
  from .generate_face_draft_validity import request
  path=HERE/'fixtures/face_draft_validity.json';q=json.loads(path.read_text());self.assertEqual(q,request());validate_operations(q)
  r=json.loads((HERE/'observations/face_draft_validity.json').read_text())
  self.assertEqual(r['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
  self.assertEqual([row['id']for row in r['results']],[op['id']for op in q['operations']])
  self.assertEqual(len(r['results']),18);self.assertTrue(all(row['value']['status']=='valid'for row in r['results']))
  for name,sha in r['provenance']['sources'].items():
   data=(HERE/name).read_bytes()
   if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
   self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
 def test_weld_controls_use_exact_seams_and_preserve_unrelated_unwelded_edges(self):
  response=json.loads((HERE/'observations/face_create_variants.json').read_text())
  counts={r['id']:len(r['value']['after_script'][0]['mesh']['vertices'])for r in response['results']}
  self.assertEqual(counts,dict(flat_zero=6,fold_44=8,fold_45=8,fold_46=6,near_seam=8,far_seam=8,old_unwelded=10))
  for name in ['near_seam','far_seam']:self.assertNotEqual(probe.VARIANTS[name]['points'][0],[4.,0.,0.])
 def test_only_bounded_fixed_private_requests_can_launch(self):
  for mutation in [{'id':'arbitrary'},{'case':'arbitrary'},{'script':'arbitrary'},{'points':[]}]:
   q=probe.request();q['operations'][0].update(mutation)
   with self.assertRaises(ValueError):validate_operations(q)
  with mock.patch.dict(os.environ,{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as spawn:
   with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(probe.request())
   client=OracleClient(settings_scheme='VibocerosOracleFaceCreateTest20261021')
   for kind in ['repeat','boolean','count','duplicate','reordered_controls','mixed_controls','reordered_drafts','mixed_drafts','reordered_extras','reordered_corners','mixed_corners','reordered_corner_controls','reordered_corner_finish']:
    q=probe.request()
    if kind=='repeat':q['iterations']=2
    elif kind=='boolean':q['iterations']=True
    elif kind=='count':q['operations']*=2
    elif kind=='duplicate':q['operations'].append(copy.deepcopy(q['operations'][0]))
    elif kind=='reordered_controls':q=probe.control_request();q['operations'].reverse()
    elif kind=='mixed_controls':q=probe.control_request();q['operations'].append(probe.request()['operations'][0])
    elif kind=='reordered_drafts':q=draft_request();q['operations'].reverse()
    elif kind=='reordered_extras':q=extra_request();q['operations'].reverse()
    elif kind=='mixed_drafts':q=draft_request();q['operations'].append(probe.request()['operations'][0])
    elif kind=='reordered_corners':q=corner_request();q['operations'].reverse()
    elif kind=='mixed_corners':q=corner_request();q['operations'].append(probe.request()['operations'][0])
    elif kind=='reordered_corner_controls':q=corner_control_request();q['operations'].reverse()
    else:q=corner_finish_request();q['operations'].reverse()
    with self.assertRaises(OracleProtocolError):client.run_rhino(q)
   spawn.assert_not_called()
 def test_helper_staging_closes_worker_dependencies(self):
  with tempfile.TemporaryDirectory()as directory:
   stage_helpers(probe.control_request(),HERE,Path(directory))
   for name in ['face_create_probe','face_create_input','face_draft_cases','face_draft_input','join_probe','merge_edges_probe']:
    self.assertEqual((Path(directory)/(name+'.py')).read_bytes(),(HERE/(name+'.py')).read_bytes())
 def lines(self):
  report=json.loads((ROOT/'docs/face-create-qualification.json').read_text())
  lines=[]
  for group in report['groups'].values():
   for stage in ['command','getter','controls','draft_command','draft_getter']:
    if stage in group:lines.extend(('FACE_'+stage.upper()if stage.startswith('draft_')else'FACE_CREATE_'+stage.upper())+' '+json.dumps(row)for row in group[stage]['cases'])
  return report,lines
 def test_complete_historical_qualification_preserves_limits(self):
  report,lines=self.lines();self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(lines)),report)
  self.assertEqual(report['recipes'],73);self.assertEqual(report['model_epsilon'],1e-6)
  self.assertFalse(report['full_native_parity']);self.assertFalse(report['performance_qualified'])
  gaps={id_ for group in report['groups'].values()for stage in ['draft_command','draft_getter']if stage in group for id_ in group[stage]['unqualified_recipes']}
  self.assertEqual(gaps,set())
  self.assertTrue(all(status=='valid'for status in report['native_record_validity'].values()))
  self.assertGreater(verify_measurement(report), 0)
 def test_report_rejects_missing_duplicate_invalid_errors_and_hidden_topology(self):
  report,lines=self.lines()
  for invalid in [lines[:-1],lines+lines[-1:]]:
   with self.assertRaises(ValueError):summarize('\n'.join(invalid))
  for error in [True,-1.,1.1e-6,float('nan'),float('inf')]:
   row=copy.deepcopy(report['groups']['base']['command']['cases'][0]);row['check']['meshes'][0]['maximum_bidirectional_sample_error']=error
   invalid=lines[:];invalid[0]='FACE_CREATE_COMMAND '+json.dumps(row)
   with self.assertRaises(ValueError):summarize('\n'.join(invalid))
  for key in ['actual_vertices','native_faces']:
   row=copy.deepcopy(report['groups']['base']['command']['cases'][0]);row['check']['meshes'][0][key]+=1
   invalid=lines[:];invalid[0]='FACE_CREATE_COMMAND '+json.dumps(row)
   with self.assertRaises(ValueError):summarize('\n'.join(invalid))

 def test_report_rejects_a_hidden_chain_gap_or_unrecognized_physical_control(self):
  report,lines=self.lines()
  index=next(i for i,line in enumerate(lines)if line.startswith('FACE_DRAFT_GETTER ')and json.loads(line.split(' ',1)[1])['id']=='chain_wrap')
  row=json.loads(lines[index].split(' ',1)[1]);row['check']['meshes'][0]['maximum_bidirectional_sample_error']=1.1e-6;row['qualified']=True
  invalid=lines[:];invalid[index]='FACE_DRAFT_GETTER '+json.dumps(row)
  with self.assertRaises(ValueError):summarize('\n'.join(invalid))
  extra=copy.deepcopy(report['groups']['controls']['controls']['cases'][0]);extra['id']='unexpected'
  with self.assertRaises(ValueError):summarize('\n'.join(lines+['FACE_CREATE_CONTROLS '+json.dumps(extra)]))
