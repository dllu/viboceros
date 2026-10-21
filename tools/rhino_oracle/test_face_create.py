"""Fixed 3DFace inputs, immutable oracle evidence and complete qualification."""
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
from .structural_sdk import stage_helpers, validate_operations
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]


class FaceCreateTests(unittest.TestCase):
 def test_native_recipes_and_immutable_capture_sources(self):
  for (suffix,count),factory in zip(GROUPS,[probe.request,probe.variant_request,probe.control_request]):
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
    if suffix=='_controls':
     self.assertGreater(value['input_trace']['requests'],0)
     self.assertTrue(all(p.startswith('Pick point')for p in value['input_trace']['prompts']))
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
   for kind in ['repeat','boolean','count','duplicate','reordered_controls','mixed_controls']:
    q=probe.request()
    if kind=='repeat':q['iterations']=2
    elif kind=='boolean':q['iterations']=True
    elif kind=='count':q['operations']*=2
    elif kind=='duplicate':q['operations'].append(copy.deepcopy(q['operations'][0]))
    elif kind=='reordered_controls':q=probe.control_request();q['operations'].reverse()
    else:q=probe.control_request();q['operations'].append(probe.request()['operations'][0])
    with self.assertRaises(OracleProtocolError):client.run_rhino(q)
   spawn.assert_not_called()
 def test_helper_staging_closes_worker_dependencies(self):
  with tempfile.TemporaryDirectory()as directory:
   stage_helpers(probe.control_request(),HERE,Path(directory))
   for name in ['face_create_probe','face_create_input','join_probe','merge_edges_probe']:
    self.assertEqual((Path(directory)/(name+'.py')).read_bytes(),(HERE/(name+'.py')).read_bytes())
 def lines(self):
  report=json.loads((ROOT/'docs/face-create-qualification.json').read_text())
  lines=[]
  for group in report['groups'].values():
   for stage in ['command','getter','controls']:
    if stage in group:lines.extend('FACE_CREATE_'+stage.upper()+' '+json.dumps(row)for row in group[stage]['cases'])
  return report,lines
 def test_complete_current_qualification_preserves_limits(self):
  report,lines=self.lines();self.assertEqual(summarize('\n'.join(lines)),report)
  self.assertEqual(report['recipes'],30);self.assertEqual(report['model_epsilon'],1e-6)
  self.assertFalse(report['full_native_parity']);self.assertFalse(report['performance_qualified'])
  for path,sha in report['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),sha,path)
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
