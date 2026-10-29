"""Fixed native SubDCylinder inputs, archived producers and qualification guards."""
from tools.verify_provenance import reconstruct_measurement, verify_measurement
import copy,gzip,hashlib,json,unittest,tempfile,subprocess,sys
from pathlib import Path
from . import subd_cylinder_probe as probe
from .report_subd_cylinder import HERE,ROOT,summarize
from .structural_sdk import validate_operations,stage_helpers
class SubDCylinderTests(unittest.TestCase):
 def test_inputs_sources_and_native_diagnostics(self):
  path=HERE/'fixtures/subd_cylinder.json';q=json.loads(path.read_text());self.assertEqual(q,probe.request());validate_operations(q);native=json.loads((HERE/'observations/subd_cylinder.json').read_text());self.assertEqual(len(native['results']),28);self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest());self.assertEqual([r['id']for r in native['results']],[o['id']for o in q['operations']])
  for source,sha in native['provenance']['sources'].items():
   data=(HERE/source).read_bytes()
   if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
   self.assertEqual(hashlib.sha256(data).hexdigest(),sha,source)
  for row in native['results']:
   v=row['value'];self.assertEqual(v['input'],probe.SCENES[row['id']]);self.assertEqual(len([e for e in v['events']if e['name']=='SubDCylinder']),1)
   if v['input'].get('unsupported'):self.assertIn('Unknown command:',v['history'])
   else:self.assertNotIn('Unknown command:',v['history']);self.assertEqual(v['undo'],v['before']);self.assertEqual(v['redo'],v['after_script'])
 def test_standalone_staging_and_modified_recipes_are_checked_before_launch(self):
  q=probe.request()
  with tempfile.TemporaryDirectory()as directory:
   job=Path(directory);stage_helpers(q,HERE,job)
   self.assertEqual({p.name for p in job.iterdir()},{'subd_cylinder_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py'})
   subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import subd_cylinder_probe,subd_box_probe,join_probe,merge_edges_probe',str(job)],check=True,capture_output=True,text=True)
  for edit in [dict(case='arbitrary'),dict(extra=True),dict(id='different')]:
   bad=copy.deepcopy(q);bad['operations'][0].update(edit)
   with self.assertRaises(ValueError):validate_operations(bad)
 def lines(self):
  report=json.loads((ROOT/'docs/subd-cylinder-qualification.json').read_text());return report,['SUBD_CYLINDER_'+stage.upper()+' '+json.dumps(row)for stage,rows in report['stages'].items()for row in rows]
 def test_complete_historical_report(self):
  report,lines=self.lines();self.assertEqual(report,reconstruct_measurement(report, summarize, '\n'.join(lines)));self.assertFalse(report['full_native_parity']);self.assertFalse(report['performance_qualified'])
  self.assertGreater(verify_measurement(report), 0)
 def test_missing_failed_or_invalid_data_cannot_qualify(self):
  report,lines=self.lines()
  for data in [lines[:-1],lines+lines[-1:],lines[::-1]]:
   with self.assertRaises(ValueError):summarize('\n'.join(data))
  for key,bad in [('qualified',False),('maximum_control_or_limit_point_error',True),('maximum_sampled_surface_error',float('nan')),('maximum_sampled_surface_error',1.1e-6),('faces',0),('surface_sampled',False)]:
   row=copy.deepcopy(report['stages']['command'][0]);row['check']['objects'][0][key]=bad;data=lines[:];data[0]='SUBD_CYLINDER_COMMAND '+json.dumps(row)
   with self.assertRaises(ValueError):summarize('\n'.join(data))
if __name__=='__main__':unittest.main()
