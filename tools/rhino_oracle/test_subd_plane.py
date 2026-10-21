"""Fixed native SubDPlane inputs, archived producers and qualification guards."""
import copy,gzip,hashlib,json,unittest
from . import subd_plane_probe as probe
from .report_subd_plane import HERE,ROOT,summarize
from .structural_sdk import validate_operations
class SubDPlaneTests(unittest.TestCase):
 def test_inputs_sources_and_native_diagnostics(self):
  path=HERE/'fixtures/subd_plane.json';q=json.loads(path.read_text());self.assertEqual(q,probe.request());validate_operations(q);native=json.loads((HERE/'observations/subd_plane.json').read_text());self.assertEqual(len(native['results']),19);self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest());self.assertEqual([r['id']for r in native['results']],[o['id']for o in q['operations']])
  for source,sha in native['provenance']['sources'].items():
   data=(HERE/source).read_bytes()
   if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
   self.assertEqual(hashlib.sha256(data).hexdigest(),sha,source)
  for row in native['results']:
   v=row['value'];self.assertEqual(v['input'],probe.SCENES[row['id']]);self.assertEqual(len([e for e in v['events']if e['name']=='SubDPlane']),1)
   if v['input'].get('unsupported'):self.assertIn('Unknown command:',v['history'])
   else:self.assertNotIn('Unknown command:',v['history']);self.assertEqual(v['undo'],v['before']);self.assertEqual(v['redo'],v['after_script'])
 def lines(self):
  report=json.loads((ROOT/'docs/subd-plane-qualification.json').read_text());return report,['SUBD_PLANE_'+stage.upper()+' '+json.dumps(row)for stage,rows in report['stages'].items()for row in rows]
 def test_complete_current_report(self):
  report,lines=self.lines();self.assertEqual(report,summarize('\n'.join(lines)));self.assertFalse(report['full_native_parity']);self.assertFalse(report['performance_qualified'])
  for path,sha in report['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),sha,path)
 def test_missing_failed_or_invalid_data_cannot_qualify(self):
  report,lines=self.lines()
  for data in [lines[:-1],lines+lines[-1:],lines[::-1]]:
   with self.assertRaises(ValueError):summarize('\n'.join(data))
  for key,bad in [('qualified',False),('maximum_control_or_limit_point_error',True),('maximum_sampled_surface_error',float('nan')),('maximum_sampled_surface_error',1.1e-6),('faces',0),('surface_sampled',False)]:
   row=copy.deepcopy(report['stages']['command'][0]);row['check']['objects'][0][key]=bad;data=lines[:];data[0]='SUBD_PLANE_COMMAND '+json.dumps(row)
   with self.assertRaises(ValueError):summarize('\n'.join(data))
if __name__=='__main__':unittest.main()
