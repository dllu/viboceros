"""Control qualification rejects lost input evidence and out-of-gate replays."""
import copy,gzip,json,shutil,tempfile,unittest
from pathlib import Path
from unittest import mock
from . import blend_curve_controls_evidence as evidence,report_blend_curve_controls as report
from .check_blend_curve_controls import ROOT,PROOF
from .client import OracleClient,OracleProtocolError
class ControlBlendTests(unittest.TestCase):
 def test_inventory_and_fourth_policy_are_reproducible(self):
  workflows,captures,counts,policy,paths=evidence.collect(PROOF)
  self.assertEqual((len(workflows),len(captures)),(77,8))
  self.assertEqual(counts['cancelled_workflows'],1);self.assertEqual(counts['joined_workflows'],1)
  self.assertEqual(counts['point_tangent_stations'],76*33)
  self.assertLess(policy['maximum_native_fourth_policy_residual'],1e-6)
  self.assertGreater(policy['long_rational_domain']['native_domain_minus_length'],1e-5)
  self.assertIn(evidence.prior.SECONDARY,workflows);self.assertGreater(len(paths),1900)
 def test_numeric_timeouts_cannot_be_promoted_to_workflows(self):
  original=evidence.captures
  def changed(*a):
   rows,summary,paths=original(*a);v=rows[next(iter(evidence.DIAGNOSTICS))];v['input_trace'][0]['errors']=[];return rows,summary,paths
  with mock.patch.object(evidence,'captures',side_effect=changed):
   with self.assertRaisesRegex(ValueError,'numeric diagnostic'):evidence.collect(PROOF)
 def test_changed_physical_receipts_or_surface_parents_are_rejected(self):
  original=evidence.captures;key='blend_curve_control_holdouts/control_holdout_g4_second_2'
  def changed(mutate):
   rows,c,p=original(PROOF);mutate(rows[key]);return rows,c,p
  def parent(v):
   for phase in ['after','redo']:v[phase][1]['surfaces'][0]['control_points'][0]['point'][0]+=1
  for mutate,message in [(lambda v:v['input_trace'][0]['clicks'][-1].update(click_acknowledged=False),'public click'),(lambda v:v['input_trace'][0].update(completed=0),'incomplete control'),(parent,'surface parent')]:
   with mock.patch.object(evidence,'captures',side_effect=lambda *a:changed(mutate)):
    with self.assertRaisesRegex(ValueError,message):evidence.collect(PROOF)
 def test_terminal_receipts_and_source_digests_are_required(self):
  with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
   proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'captures.json';original=p.read_bytes();rows=json.loads(original);rows[0]['exit_status']=143;p.write_text(json.dumps(rows))
   with self.assertRaisesRegex(ValueError,'terminal receipt'):evidence.collect(proof)
   rows=json.loads(original);rows[0]['observation_sha256']='0'*64;p.write_text(json.dumps(rows))
   with self.assertRaisesRegex(ValueError,'capture digest'):evidence.collect(proof)
 def test_private_settings_and_single_fixed_recipes_are_enforced(self):
  with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
   for names in evidence.FAMILIES.values():
    query=evidence.recipes.request(names)
    with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(query)
    query=copy.deepcopy(query);query['operations']*=2
    with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleControlInvalid').run_rhino(query)
   launch.assert_not_called()
 def test_pinned_record_rejects_missing_or_out_of_gate_replays(self):
  path=ROOT/'docs/blend-curve-controls-validation.json'
  if not path.exists():self.skipTest('record follows implementation commit')
  value=json.loads(path.read_bytes());revision=value['implementation_revision'];self.assertEqual(value,report.build(PROOF,revision))
  with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
   proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'controls-ui-portable.log.gz';lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True);i=next(i for i,line in enumerate(lines)if line.startswith('BLEND_CURVE_CONTROLS_UI '));row=json.loads(lines[i].split(' ',1)[1])
   for field,error,message in [('maximum_coefficient_and_sample_error',1.1e-6,'geometry gate'),('maximum_normalized_derivative_error',float('nan'),'geometry gate'),('maximum_parameter_domain_gate_ratio',1.1,'fractional domain'),('nominal_capture',False,'classification'),('native_stations',0,'classification'),('archives',0,'archive gate')]:
    changed=lines.copy();changed[i]='BLEND_CURVE_CONTROLS_UI '+json.dumps(dict(row,**{field:error}))+'\n';p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
    with self.assertRaisesRegex(ValueError,message):report.build(proof,revision)
   p.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
   with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.build(proof,revision)
if __name__=='__main__':unittest.main()
