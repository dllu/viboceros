"""Guard capture lifetimes, public pixels and strict curvature comparison gates."""
import gzip,json,shutil,tempfile,unittest
from pathlib import Path
from unittest import mock
from . import blend_curve_display_evidence as evidence,report_blend_curve_display as report
from .check_blend_curve_display import ROOT,PROOF

class DisplayBlendTests(unittest.TestCase):
 def test_corpus_settings_and_continuation_sequences_are_reproducible(self):
  rows,captures,counts,paths=evidence.collect(PROOF)
  self.assertEqual((len(rows),len(captures)),(26,6))
  self.assertEqual(counts,dict(accepted_workflows=22,cancelled_workflows=4,physical_picks=50,point_tangent_stations=726,derivative_stations=372,curvature_stations=3762,viewport_snapshots=60,analysis_snapshots=57))
  self.assertGreater(len(paths),50)
 def test_changed_analysis_input_acknowledgements_or_viewport_pixels_are_rejected(self):
  original=evidence.captures
  def changed(mutate):
   rows,c,p=original(PROOF,evidence.FAMILIES);mutate(rows['blend_curve_display_lifecycle/display_enable_lifetime']);return rows,c,p
  def mode(v):
   v['completion_observation'][0]['object_graphs'][1]['enabled']=True
  changes=[(mode,'transient analysis lifetime'),(lambda v:v['input_trace'][0]['clicks'][0].update(click_acknowledged=False),'public input receipt'),(lambda v:v['input_trace'][0]['viewport_snapshots'][0].update(png_base64='AAAA'),'viewport evidence'),(lambda v:v['input_trace'][0]['viewport_snapshots'][0]['settings'].update(scale=111),'graph settings')]
  for mutate,message in changes:
   with mock.patch.object(evidence,'captures',side_effect=lambda *args:changed(mutate)):
    with self.assertRaisesRegex(ValueError,message):evidence.collect(PROOF)
 def test_interrupted_capture_cannot_be_counted_as_published_observations(self):
  with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
   proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'capture-attempts.json';rows=json.loads(p.read_bytes());rows[0]['native_observations']=1;p.write_text(json.dumps(rows))
   with self.assertRaisesRegex(ValueError,'interrupted capture'):evidence.collect(proof)
 def test_missing_nonfinite_and_out_of_gate_comparisons_are_rejected(self):
  with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
   proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'display-command-portable.log.gz';lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True);i=next(i for i,line in enumerate(lines)if line.startswith('BLEND_CURVE_DISPLAY_COMMAND '));row=json.loads(lines[i].split(' ',1)[1])
   for field,error,message in [('maximum_curvature_vector_error',1.1e-6,'geometry gate'),('maximum_scaled_curvature_tip_error',float('nan'),'geometry gate'),('maximum_parameter_domain_gate_ratio',1.1,'domain or derivative'),('curvature_stations',1,'classification'),('archives',0,'archive gate')]:
    changed=lines.copy();changed[i]='BLEND_CURVE_DISPLAY_COMMAND '+json.dumps(dict(row,**{field:error}))+'\n';p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
    with self.assertRaisesRegex(ValueError,message):report.comparisons(proof)
   p.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
   with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.comparisons(proof)
 def test_pinned_record_reproduces(self):
  path=ROOT/'docs/blend-curve-display-validation.json'
  if not path.exists():self.skipTest('record follows implementation commit')
  value=json.loads(path.read_bytes());self.assertEqual(value,report.build(PROOF,value['implementation_revision']))

if __name__=='__main__':unittest.main()
