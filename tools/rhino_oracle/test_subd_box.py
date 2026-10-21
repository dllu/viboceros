"""Immutable native SubDBox inputs, producer snapshots and report guards."""
import copy,gzip,hashlib,json,unittest
from . import subd_box_probe as probe
from .report_subd_box import HERE,ROOT,summarize
from .structural_sdk import validate_operations
class SubDBoxTests(unittest.TestCase):
 def test_inputs_sources_and_native_diagnostics(self):
  path=HERE/'fixtures/subd_box.json';request=json.loads(path.read_text());self.assertEqual(request,probe.request());validate_operations(request)
  native=json.loads((HERE/'observations/subd_box.json').read_text());self.assertEqual(len(native['results']),23);self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest());self.assertEqual([r['id']for r in native['results']],[o['id']for o in request['operations']])
  for source,sha in native['provenance']['sources'].items():
   data=(HERE/source).read_bytes()
   if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
   self.assertEqual(hashlib.sha256(data).hexdigest(),sha,source)
  for row in native['results']:
   v=row['value'];self.assertEqual(v['input'],probe.SCENES[row['id']]);self.assertEqual(len([e for e in v['events']if e['name']=='SubDBox']),1)
   if v['input'].get('unsupported'):self.assertIn('Unknown command:',v['history'])
   else:
    self.assertNotIn('Unknown command:',v['history']);self.assertEqual(v['undo'],v['before']);self.assertEqual(v['redo'],v['after_script'])
 def lines(self):
  report=json.loads((ROOT/'docs/subd-box-qualification.json').read_text());return report,['SUBD_BOX_'+stage.upper()+' '+json.dumps(row)for stage,rows in report['stages'].items()for row in rows]
 def test_complete_current_report(self):
  report,lines=self.lines();self.assertEqual(report,summarize('\n'.join(lines)));self.assertFalse(report['full_native_parity']);self.assertFalse(report['performance_qualified'])
  for path,sha in report['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),sha,path)
 def test_missing_failed_or_invalid_data_cannot_qualify(self):
  report,lines=self.lines()
  for bad in [lines[:-1],lines+lines[-1:],lines[::-1]]:
   with self.assertRaises(ValueError):summarize('\n'.join(bad))
  for key,bad in [('qualified',False),('maximum_control_or_limit_point_error',True),('maximum_sampled_surface_error',float('nan')),('maximum_sampled_surface_error',1.1e-6),('faces',0),('surface_sampled',False)]:
   row=copy.deepcopy(report['stages']['command'][0]);row['check']['objects'][0][key]=bad;data=lines[:];data[0]='SUBD_BOX_COMMAND '+json.dumps(row)
   with self.assertRaises(ValueError):summarize('\n'.join(data))
  row=copy.deepcopy(report['stages']['command'][0]);row['grouped_history']=False;data=lines[:];data[0]='SUBD_BOX_COMMAND '+json.dumps(row)
  with self.assertRaises(ValueError):summarize('\n'.join(data))
if __name__=='__main__':unittest.main()
